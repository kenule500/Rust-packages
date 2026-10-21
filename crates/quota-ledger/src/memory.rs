use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::policy;
use crate::store::QuotaStore;
use crate::{
    Budget, ExhaustionPolicy, Metric, ProviderId, QuotaError, QuotaKey, Reservation, ReservationId,
    ReserveOutcome, ReserveRequest, TenantId, Usage, Window,
};

type BudgetKey = (TenantId, ProviderId, Metric);

const LOCK_POISONED: &str = "quota store lock poisoned";

/// An in-memory [`QuotaStore`] for tests and single-process services.
///
/// All mutation happens under one write lock, so reserve, commit, release, and
/// usage are atomic with respect to each other. Expired reservations are
/// reclaimed on the next reserve or usage call, and by
/// [`MemoryQuotaStore::reclaim_expired`].
#[derive(Default)]
pub struct MemoryQuotaStore {
    ledgers: RwLock<HashMap<BudgetKey, Ledger>>,
}

impl MemoryQuotaStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of ledger buckets.
    pub fn len(&self) -> usize {
        self.ledgers.read().map(|guard| guard.len()).unwrap_or(0)
    }

    /// Returns `true` when no ledger buckets exist.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reclaims all reservations whose TTL has elapsed.
    ///
    /// Returns the number of reservations reclaimed.
    pub fn reclaim_expired(&self) -> usize {
        let Ok(mut guard) = self.ledgers.write() else {
            return 0;
        };
        let now = Utc::now();
        guard
            .values_mut()
            .map(|ledger| {
                ledger.reset_if_expired(now);
                ledger.reclaim(now)
            })
            .sum()
    }
}

impl fmt::Debug for MemoryQuotaStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryQuotaStore")
            .field("ledgers", &self.len())
            .finish()
    }
}

#[async_trait]
impl QuotaStore for MemoryQuotaStore {
    async fn reserve(&self, request: &ReserveRequest) -> Result<ReserveOutcome, QuotaError> {
        if request.units == 0 {
            return Err(QuotaError::Config(
                "a reservation must hold at least one unit".to_owned(),
            ));
        }
        if request.ttl <= Duration::zero() {
            return Err(QuotaError::Config(
                "a reservation TTL must be positive".to_owned(),
            ));
        }

        let now = Utc::now();
        let mut guard = self
            .ledgers
            .write()
            .map_err(|_| QuotaError::Backend(LOCK_POISONED.to_owned()))?;
        let budget_key = (
            request.budget.tenant,
            request.budget.provider,
            request.budget.metric,
        );
        let ledger = guard
            .entry(budget_key)
            .or_insert_with(|| Ledger::new(&request.budget, now));
        ledger.sync(&request.budget, now);
        ledger.reclaim(now);

        if let Some(idempotency_key) = request.idempotency_key.as_ref() {
            if let Some(existing_id) = ledger.idempotency.get(idempotency_key).copied() {
                let state = ledger.reservations.get(&existing_id).ok_or_else(|| {
                    QuotaError::Backend("quota idempotency index is inconsistent".to_owned())
                })?;
                if state.reservation.units != request.units {
                    return Err(QuotaError::IdempotencyConflict(format!(
                        "idempotency key `{idempotency_key}` was first used for {} units, not {}",
                        state.reservation.units, request.units
                    )));
                }
                let reservation = state.reservation.clone();
                return Ok(if state.overage > 0 {
                    ReserveOutcome::Overage {
                        reservation,
                        cost_cents: state.overage_cents,
                    }
                } else {
                    ReserveOutcome::Reserved(reservation)
                });
            }
        }

        let remaining = ledger
            .limit
            .saturating_sub(ledger.used.saturating_add(ledger.held));

        if request.units <= remaining {
            let reservation = ledger.hold(&request.budget, request, 0, 0, now);
            return Ok(ReserveOutcome::Reserved(reservation));
        }

        match request.budget.policy {
            ExhaustionPolicy::HardStop => {
                Ok(policy::denial(request.units, remaining, ledger.window_end))
            }
            ExhaustionPolicy::Degrade => Ok(policy::degraded(ledger.window_end)),
            ExhaustionPolicy::Queue => Ok(policy::queued(ledger.window_end)),
            ExhaustionPolicy::Overage { unit_cost_cents } => {
                let cost_cents = request.units.saturating_mul(u64::from(unit_cost_cents));
                let reservation =
                    ledger.hold(&request.budget, request, request.units, cost_cents, now);
                Ok(ReserveOutcome::Overage {
                    reservation,
                    cost_cents,
                })
            }
        }
    }

    async fn commit(&self, id: ReservationId) -> Result<(), QuotaError> {
        let mut guard = self
            .ledgers
            .write()
            .map_err(|_| QuotaError::Backend(LOCK_POISONED.to_owned()))?;

        for ledger in guard.values_mut() {
            if let Some(state) = ledger.reservations.get(&id) {
                if state.settled {
                    return Ok(());
                }
                let units = state.reservation.units;
                if let Some(state) = ledger.reservations.get_mut(&id) {
                    state.settled = true;
                }
                ledger.used = ledger.used.saturating_add(units);
                ledger.held = ledger.held.saturating_sub(units);
                return Ok(());
            }
        }

        Err(QuotaError::ReservationNotFound(id.to_string()))
    }

    async fn release(&self, id: ReservationId) -> Result<(), QuotaError> {
        let mut guard = self
            .ledgers
            .write()
            .map_err(|_| QuotaError::Backend(LOCK_POISONED.to_owned()))?;

        for ledger in guard.values_mut() {
            if let Some(state) = ledger.reservations.get(&id) {
                if state.settled {
                    return Ok(());
                }
                let units = state.reservation.units;
                let overage = state.overage;
                let overage_cents = state.overage_cents;
                if let Some(state) = ledger.reservations.get_mut(&id) {
                    state.settled = true;
                }
                ledger.held = ledger.held.saturating_sub(units);
                ledger.overage = ledger.overage.saturating_sub(overage);
                ledger.overage_cents = ledger.overage_cents.saturating_sub(overage_cents);
                return Ok(());
            }
        }

        Err(QuotaError::ReservationNotFound(id.to_string()))
    }

    async fn usage(&self, key: &QuotaKey) -> Result<Usage, QuotaError> {
        let now = Utc::now();
        let mut guard = self
            .ledgers
            .write()
            .map_err(|_| QuotaError::Backend(LOCK_POISONED.to_owned()))?;
        let budget_key = (key.tenant, key.provider, key.metric);
        let ledger = guard
            .get_mut(&budget_key)
            .ok_or_else(|| QuotaError::UnknownBudget(format!("{}/{}", key.tenant, key.provider)))?;
        ledger.reset_if_expired(now);
        ledger.reclaim(now);

        Ok(Usage {
            limit: ledger.limit,
            used: ledger.used,
            held: ledger.held,
            overage: ledger.overage,
            overage_cents: ledger.overage_cents,
            reset_at: ledger.window_end,
        })
    }
}

struct Ledger {
    limit: u64,
    policy: ExhaustionPolicy,
    window: Window,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    used: u64,
    held: u64,
    overage: u64,
    overage_cents: u64,
    reservations: HashMap<ReservationId, ReservationState>,
    idempotency: HashMap<String, ReservationId>,
}

struct ReservationState {
    reservation: Reservation,
    settled: bool,
    overage: u64,
    overage_cents: u64,
}

impl Ledger {
    fn new(budget: &Budget, now: DateTime<Utc>) -> Self {
        Self {
            limit: budget.limit,
            policy: budget.policy,
            window: budget.window,
            window_start: budget.window.start_at(now),
            window_end: budget.window.reset_at(now),
            used: 0,
            held: 0,
            overage: 0,
            overage_cents: 0,
            reservations: HashMap::new(),
            idempotency: HashMap::new(),
        }
    }

    fn sync(&mut self, budget: &Budget, now: DateTime<Utc>) {
        if now >= self.window_end || self.window != budget.window {
            self.reset(budget.window, now);
        }
        self.limit = budget.limit;
        self.policy = budget.policy;
    }

    fn reset_if_expired(&mut self, now: DateTime<Utc>) {
        if now >= self.window_end {
            self.reset(self.window, now);
        }
    }

    fn reset(&mut self, window: Window, now: DateTime<Utc>) {
        self.window = window;
        self.window_start = window.start_at(now);
        self.window_end = window.reset_at(now);
        self.used = 0;
        self.held = 0;
        self.overage = 0;
        self.overage_cents = 0;
        self.reservations.clear();
        self.idempotency.clear();
    }

    fn hold(
        &mut self,
        budget: &Budget,
        request: &ReserveRequest,
        overage: u64,
        overage_cents: u64,
        now: DateTime<Utc>,
    ) -> Reservation {
        let reservation = Reservation {
            id: ReservationId::generate(),
            key: QuotaKey::new(
                budget.tenant,
                budget.provider,
                budget.metric,
                self.window_start,
            ),
            units: request.units,
            idempotency_key: request.idempotency_key.clone(),
            expires_at: now
                .checked_add_signed(request.ttl)
                .unwrap_or(DateTime::<Utc>::MAX_UTC),
            policy: budget.policy,
        };

        self.held = self.held.saturating_add(request.units);
        self.overage = self.overage.saturating_add(overage);
        self.overage_cents = self.overage_cents.saturating_add(overage_cents);
        self.reservations.insert(
            reservation.id,
            ReservationState {
                reservation: reservation.clone(),
                settled: false,
                overage,
                overage_cents,
            },
        );
        if let Some(idempotency_key) = request.idempotency_key.as_ref() {
            self.idempotency
                .insert(idempotency_key.clone(), reservation.id);
        }
        reservation
    }

    fn reclaim(&mut self, now: DateTime<Utc>) -> usize {
        let expired: Vec<(ReservationId, u64, u64, u64)> = self
            .reservations
            .iter()
            .filter(|(_, state)| !state.settled && state.reservation.expires_at <= now)
            .map(|(id, state)| {
                (
                    *id,
                    state.reservation.units,
                    state.overage,
                    state.overage_cents,
                )
            })
            .collect();

        let reclaimed = expired.len();
        for (id, units, overage, overage_cents) in expired {
            if let Some(state) = self.reservations.get_mut(&id) {
                state.settled = true;
            }
            self.held = self.held.saturating_sub(units);
            self.overage = self.overage.saturating_sub(overage);
            self.overage_cents = self.overage_cents.saturating_sub(overage_cents);
        }
        reclaimed
    }
}
