use std::fmt;

use chrono::{DateTime, FixedOffset, Utc};
use uuid::Uuid;

pub use platform_core::{ProviderId, TenantId};

/// The unit a budget is measured in, for example `requests` or `messages`.
///
/// Metric keys are opaque to the crate and owned by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Metric(&'static str);

impl Metric {
    /// Creates a metric key.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the metric key.
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// The period of a fixed quota window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// The window resets at local midnight every day.
    Daily,
    /// The window resets at local midnight on the first day of the next month.
    Monthly,
}

/// How a quota window is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// A calendar-aligned window in a fixed UTC offset.
    Fixed {
        /// The calendar period.
        period: Period,
        /// The provider's UTC offset.
        timezone: FixedOffset,
    },
    /// A window of a fixed length anchored at the first reservation.
    Rolling {
        /// The window length.
        duration: chrono::Duration,
    },
}

impl Window {
    /// Creates a fixed daily or monthly window.
    pub const fn fixed(period: Period, timezone: FixedOffset) -> Self {
        Self::Fixed { period, timezone }
    }

    /// Creates a rolling window of the given length.
    pub const fn rolling(duration: chrono::Duration) -> Self {
        Self::Rolling { duration }
    }

    /// Returns the instant at which the current window ends.
    pub fn reset_at(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        crate::window::reset_at(self, now)
    }

    /// Returns the instant at which the current window started.
    pub fn start_at(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        crate::window::start_at(self, now)
    }
}

/// One quota budget for a tenant, provider, and metric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    /// The tenant that owns the budget.
    pub tenant: TenantId,
    /// The provider whose quota is measured.
    pub provider: ProviderId,
    /// The measured unit.
    pub metric: Metric,
    /// The maximum units allowed in a window.
    pub limit: u64,
    /// How the window is measured.
    pub window: Window,
    /// What happens when the budget is exhausted.
    pub policy: ExhaustionPolicy,
}

impl Budget {
    /// Creates a budget.
    pub const fn new(
        tenant: TenantId,
        provider: ProviderId,
        metric: Metric,
        limit: u64,
        window: Window,
        policy: ExhaustionPolicy,
    ) -> Self {
        Self {
            tenant,
            provider,
            metric,
            limit,
            window,
            policy,
        }
    }
}

/// What the ledger does when a budget is exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExhaustionPolicy {
    /// Deny the reservation.
    HardStop,
    /// Allow the reservation and record the units and cost as overage.
    Overage {
        /// The cost of one overage unit in cents.
        unit_cost_cents: u32,
    },
    /// Return a degraded outcome without consuming units.
    Degrade,
    /// Return a queued outcome without consuming units.
    Queue,
}

/// The identity of one ledger bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QuotaKey {
    /// The tenant that owns the budget.
    pub tenant: TenantId,
    /// The provider whose quota is measured.
    pub provider: ProviderId,
    /// The measured unit.
    pub metric: Metric,
    /// The start of the window this bucket belongs to.
    pub window_start: DateTime<Utc>,
}

impl QuotaKey {
    /// Creates a ledger key.
    pub const fn new(
        tenant: TenantId,
        provider: ProviderId,
        metric: Metric,
        window_start: DateTime<Utc>,
    ) -> Self {
        Self {
            tenant,
            provider,
            metric,
            window_start,
        }
    }
}

/// The identifier of a reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReservationId(Uuid);

impl ReservationId {
    /// Generates a new time-ordered reservation identifier.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wraps an existing UUID.
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Display for ReservationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A held claim on a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    /// The reservation identifier.
    pub id: ReservationId,
    /// The ledger bucket the reservation belongs to.
    pub key: QuotaKey,
    /// The number of units held.
    pub units: u64,
    /// The caller's idempotency key, when supplied.
    pub idempotency_key: Option<String>,
    /// The instant after which an unsettled reservation is reclaimed.
    pub expires_at: DateTime<Utc>,
    /// The policy in force when the reservation was created.
    pub policy: ExhaustionPolicy,
}

/// The current state of a budget bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// The configured limit.
    pub limit: u64,
    /// Units consumed by committed reservations.
    pub used: u64,
    /// Units held by unsettled reservations.
    pub held: u64,
    /// Units consumed beyond the limit under the overage policy.
    pub overage: u64,
    /// The accumulated overage cost in cents.
    pub overage_cents: u64,
    /// The instant at which the window resets.
    pub reset_at: DateTime<Utc>,
}

/// A request to reserve units from a budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReserveRequest {
    /// The budget being spent.
    pub budget: Budget,
    /// The number of units to reserve.
    pub units: u64,
    /// The caller's idempotency key, when supplied.
    pub idempotency_key: Option<String>,
    /// How long the hold survives before it is reclaimed.
    pub ttl: chrono::Duration,
}

impl ReserveRequest {
    /// Creates a request with a five-minute hold.
    pub fn new(budget: Budget, units: u64) -> Self {
        Self {
            budget,
            units,
            idempotency_key: None,
            ttl: chrono::Duration::minutes(5),
        }
    }

    /// Sets the caller idempotency key.
    pub fn with_idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Sets the hold lifetime.
    pub fn with_ttl(mut self, ttl: chrono::Duration) -> Self {
        self.ttl = ttl;
        self
    }
}

/// A denial with the information a caller needs to explain it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaExceeded {
    /// The number of units requested.
    pub requested: u64,
    /// The number of units still available.
    pub remaining: u64,
    /// The instant at which the window resets.
    pub reset_at: DateTime<Utc>,
    /// A short, non-secret upgrade hint.
    pub upgrade_hint: &'static str,
}

/// The result of a reservation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReserveOutcome {
    /// The units were reserved normally.
    Reserved(Reservation),
    /// The units were reserved beyond the limit under the overage policy.
    Overage {
        /// The overage reservation.
        reservation: Reservation,
        /// The cost of this reservation's overage units in cents.
        cost_cents: u64,
    },
    /// The hard-stop policy denied the reservation.
    Denied(QuotaExceeded),
    /// The degrade policy returned a degraded outcome.
    Degraded {
        /// The instant at which the window resets.
        reset_at: DateTime<Utc>,
    },
    /// The queue policy returned a queued outcome.
    Queued {
        /// The instant at which the window resets.
        reset_at: DateTime<Utc>,
    },
}

impl ReserveOutcome {
    /// Returns the reservation when the outcome allows the call.
    pub fn reservation(&self) -> Option<&Reservation> {
        match self {
            Self::Reserved(reservation) | Self::Overage { reservation, .. } => Some(reservation),
            Self::Denied(_) | Self::Degraded { .. } | Self::Queued { .. } => None,
        }
    }

    /// Returns `true` when the caller may proceed with the outbound call.
    pub fn is_allowed(&self) -> bool {
        self.reservation().is_some()
    }
}
