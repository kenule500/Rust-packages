//! The optional Postgres-backed [`QuotaStore`].
//!
//! The backend stores one budget row per tenant, provider, metric, and window,
//! one reservation row per hold, and one dead-hold row per reclaimed hold.
//! Reserve runs in a transaction with a per-budget advisory lock and a
//! conditional `UPDATE` that only succeeds while capacity remains.
//!
//! Connections require TLS by default. [`PostgresQuotaStore::connect_insecure_for_tests`]
//! is the explicit, test-only escape hatch; never use it in production.
//!
//! Apply [`PostgresQuotaStore::apply_schema`] or your own migration tool before
//! serving traffic.

use std::fmt;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::policy;
use crate::store::QuotaStore;
use crate::{
    Budget, ExhaustionPolicy, QuotaError, QuotaKey, Reservation, ReservationId, ReserveOutcome,
    ReserveRequest, Usage, Window,
};

type BudgetSnapshot = (i64, i64, i64, DateTime<Utc>);
type ReservationRow = (
    Uuid,
    Uuid,
    String,
    String,
    DateTime<Utc>,
    i64,
    DateTime<Utc>,
    i64,
    i64,
);
type UsageRow = (i64, i64, i64, i64, i64, DateTime<Utc>);

/// A quota store backed by Postgres.
///
/// The store owns a `PgPool`; clones are cheap and share the same pool.
#[derive(Clone)]
pub struct PostgresQuotaStore {
    pool: PgPool,
}

impl PostgresQuotaStore {
    /// Connects to Postgres with TLS required.
    ///
    /// The connection string is parsed but never stored or rendered.
    pub async fn connect(database_url: &str) -> Result<Self, QuotaError> {
        let options = parse_options(database_url)?.ssl_mode(PgSslMode::Require);
        Self::connect_with(options).await
    }

    /// Connects to Postgres with TLS disabled.
    ///
    /// This exists for local development and integration tests only. It must
    /// never be used against a production database.
    pub async fn connect_insecure_for_tests(database_url: &str) -> Result<Self, QuotaError> {
        let options = parse_options(database_url)?.ssl_mode(PgSslMode::Disable);
        Self::connect_with(options).await
    }

    /// Wraps an existing pool.
    ///
    /// The caller is responsible for the pool's TLS configuration.
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the underlying pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Executes the bundled schema.
    ///
    /// This is a convenience for local development and tests. Production
    /// deployments should apply `src/postgres/schema.sql` through their
    /// migration tool.
    pub async fn apply_schema(&self) -> Result<(), QuotaError> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await
            .map_err(backend_error)?;
        Ok(())
    }

    async fn connect_with(options: PgConnectOptions) -> Result<Self, QuotaError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await
            .map_err(backend_error)?;
        Ok(Self { pool })
    }

    /// Reclaims every unsettled reservation whose TTL has elapsed.
    ///
    /// Reclaiming is idempotent: the reservation is settled once, recorded in
    /// `quota_dead_hold` once, and its held units are returned once. Returns
    /// the number of reservations reclaimed by this call.
    pub async fn reclaim_expired(&self) -> Result<usize, QuotaError> {
        let mut tx = self.pool.begin().await.map_err(backend_error)?;
        let rows: Vec<ReservationRow> = sqlx::query_as(
            "UPDATE quota_reservation SET settled_at = now() \
             WHERE settled_at IS NULL AND expires_at <= now() \
             RETURNING reservation_id, tenant_id, provider_id, metric, window_start, \
                       units, expires_at, overage_units, overage_cents",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(backend_error)?;

        let mut reclaimed = 0;
        for (
            reservation_id,
            tenant_id,
            provider_id,
            metric,
            window_start,
            units,
            _expires_at,
            overage_units,
            overage_cents,
        ) in rows
        {
            let inserted: Option<Uuid> = sqlx::query_scalar(
                "INSERT INTO quota_dead_hold \
                   (reservation_id, tenant_id, provider_id, metric, window_start, units, \
                    overage_units, overage_cents) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (reservation_id) DO NOTHING \
                 RETURNING reservation_id",
            )
            .bind(reservation_id)
            .bind(tenant_id)
            .bind(&provider_id)
            .bind(&metric)
            .bind(window_start)
            .bind(units)
            .bind(overage_units)
            .bind(overage_cents)
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend_error)?;

            if inserted.is_some() {
                sqlx::query(
                    "UPDATE quota_budget \
                     SET held_units = GREATEST(held_units - $5, 0), \
                         overage_units = GREATEST(overage_units - $6, 0), \
                         overage_cents = GREATEST(overage_cents - $7, 0) \
                     WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                       AND window_start = $4",
                )
                .bind(tenant_id)
                .bind(&provider_id)
                .bind(&metric)
                .bind(window_start)
                .bind(units)
                .bind(overage_units)
                .bind(overage_cents)
                .execute(&mut *tx)
                .await
                .map_err(backend_error)?;
                reclaimed += 1;
            }
        }

        tx.commit().await.map_err(backend_error)?;
        Ok(reclaimed)
    }
}

impl fmt::Debug for PostgresQuotaStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresQuotaStore")
            .field("pool", &"<redacted>")
            .finish()
    }
}

#[async_trait]
impl QuotaStore for PostgresQuotaStore {
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
        let units = to_i64(request.units, "units")?;
        let limit = to_i64(request.budget.limit, "limit")?;
        let expires_at = now
            .checked_add_signed(request.ttl)
            .unwrap_or(DateTime::<Utc>::MAX_UTC);
        let tenant = *request.budget.tenant.as_uuid();
        let provider = request.budget.provider.as_str();
        let metric = request.budget.metric.as_str();

        let mut tx = self.pool.begin().await.map_err(backend_error)?;

        let lock_key = format!("{tenant}:{provider}:{metric}");
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(&lock_key)
            .execute(&mut *tx)
            .await
            .map_err(backend_error)?;

        let (window_start, window_end) = if matches!(request.budget.window, Window::Rolling { .. })
        {
            let active: Option<(DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
                "SELECT window_start, window_end FROM quota_budget \
                 WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                   AND window_end > $4 \
                 ORDER BY window_start DESC LIMIT 1 FOR UPDATE",
            )
            .bind(tenant)
            .bind(provider)
            .bind(metric)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend_error)?;

            match active {
                Some((start, end)) => (start, end),
                None => (now, request.budget.window.reset_at(now)),
            }
        } else {
            (
                request.budget.window.start_at(now),
                request.budget.window.reset_at(now),
            )
        };

        let snapshot: BudgetSnapshot = sqlx::query_as(
            "INSERT INTO quota_budget \
               (tenant_id, provider_id, metric, window_start, window_end, limit_units, \
                policy, unit_cost_cents) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (tenant_id, provider_id, metric, window_start) DO UPDATE SET \
               window_end = EXCLUDED.window_end, \
               limit_units = EXCLUDED.limit_units, \
               policy = EXCLUDED.policy, \
               unit_cost_cents = EXCLUDED.unit_cost_cents \
             RETURNING used_units, held_units, limit_units, window_end",
        )
        .bind(tenant)
        .bind(provider)
        .bind(metric)
        .bind(window_start)
        .bind(window_end)
        .bind(limit)
        .bind(policy_name(request.budget.policy))
        .bind(policy_unit_cost(request.budget.policy))
        .fetch_one(&mut *tx)
        .await
        .map_err(backend_error)?;
        let (used_units, held_units, limit_units, snapshot_window_end) = snapshot;

        if let Some(idempotency_key) = request.idempotency_key.as_deref() {
            let existing: Option<ReservationRow> = sqlx::query_as(
                "SELECT reservation_id, tenant_id, provider_id, metric, window_start, units, \
                        expires_at, overage_units, overage_cents \
                 FROM quota_reservation \
                 WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                   AND idempotency_key = $4",
            )
            .bind(tenant)
            .bind(provider)
            .bind(metric)
            .bind(idempotency_key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend_error)?;

            if let Some(row) = existing {
                let existing_units = row.5;
                let outcome = if existing_units != units {
                    Err(QuotaError::IdempotencyConflict(format!(
                        "idempotency key `{idempotency_key}` was first used for \
                         {existing_units} units, not {}",
                        request.units
                    )))
                } else {
                    Ok(outcome_from_row(
                        row,
                        &request.budget,
                        Some(idempotency_key.to_owned()),
                    ))
                };
                tx.rollback().await.map_err(backend_error)?;
                return outcome;
            }
        }

        let reserved: Option<i64> = sqlx::query_scalar(
            "UPDATE quota_budget SET held_units = held_units + $5 \
             WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 AND window_start = $4 \
               AND used_units + held_units + $5 <= limit_units \
             RETURNING held_units",
        )
        .bind(tenant)
        .bind(provider)
        .bind(metric)
        .bind(window_start)
        .bind(units)
        .fetch_optional(&mut *tx)
        .await
        .map_err(backend_error)?;

        if reserved.is_some() {
            let reservation_id = Uuid::now_v7();
            let inserted = insert_reservation(
                &mut tx,
                reservation_id,
                request,
                window_start,
                expires_at,
                0,
                0,
            )
            .await?;
            if !inserted {
                let outcome = resolve_conflict(&mut tx, request, units).await;
                tx.rollback().await.map_err(backend_error)?;
                return outcome;
            }

            tx.commit().await.map_err(backend_error)?;
            return Ok(ReserveOutcome::Reserved(Reservation {
                id: ReservationId::from_uuid(reservation_id),
                key: QuotaKey::new(
                    request.budget.tenant,
                    request.budget.provider,
                    request.budget.metric,
                    window_start,
                ),
                units: request.units,
                idempotency_key: request.idempotency_key.clone(),
                expires_at,
                policy: request.budget.policy,
            }));
        }

        let remaining = to_u64(
            limit_units
                .saturating_sub(used_units)
                .saturating_sub(held_units),
        );

        match request.budget.policy {
            ExhaustionPolicy::HardStop => {
                tx.rollback().await.map_err(backend_error)?;
                Ok(policy::denial(
                    request.units,
                    remaining,
                    snapshot_window_end,
                ))
            }
            ExhaustionPolicy::Degrade => {
                tx.rollback().await.map_err(backend_error)?;
                Ok(policy::degraded(snapshot_window_end))
            }
            ExhaustionPolicy::Queue => {
                tx.rollback().await.map_err(backend_error)?;
                Ok(policy::queued(snapshot_window_end))
            }
            ExhaustionPolicy::Overage { unit_cost_cents } => {
                let cost_cents = request.units.saturating_mul(u64::from(unit_cost_cents));
                let cost = to_i64(cost_cents, "overage cost")?;

                sqlx::query(
                    "UPDATE quota_budget \
                     SET held_units = held_units + $5, \
                         overage_units = overage_units + $5, \
                         overage_cents = overage_cents + $6 \
                     WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                       AND window_start = $4",
                )
                .bind(tenant)
                .bind(provider)
                .bind(metric)
                .bind(window_start)
                .bind(units)
                .bind(cost)
                .execute(&mut *tx)
                .await
                .map_err(backend_error)?;

                let reservation_id = Uuid::now_v7();
                let inserted = insert_reservation(
                    &mut tx,
                    reservation_id,
                    request,
                    window_start,
                    expires_at,
                    units,
                    cost,
                )
                .await?;
                if !inserted {
                    let outcome = resolve_conflict(&mut tx, request, units).await;
                    tx.rollback().await.map_err(backend_error)?;
                    return outcome;
                }

                tx.commit().await.map_err(backend_error)?;
                Ok(ReserveOutcome::Overage {
                    reservation: Reservation {
                        id: ReservationId::from_uuid(reservation_id),
                        key: QuotaKey::new(
                            request.budget.tenant,
                            request.budget.provider,
                            request.budget.metric,
                            window_start,
                        ),
                        units: request.units,
                        idempotency_key: request.idempotency_key.clone(),
                        expires_at,
                        policy: request.budget.policy,
                    },
                    cost_cents,
                })
            }
        }
    }

    async fn commit(&self, id: ReservationId) -> Result<(), QuotaError> {
        let mut tx = self.pool.begin().await.map_err(backend_error)?;
        let row: Option<ReservationRow> = sqlx::query_as(
            "UPDATE quota_reservation SET settled_at = now() \
             WHERE reservation_id = $1 AND settled_at IS NULL \
             RETURNING reservation_id, tenant_id, provider_id, metric, window_start, \
                       units, expires_at, overage_units, overage_cents",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(backend_error)?;

        match row {
            Some((
                _reservation_id,
                tenant_id,
                provider_id,
                metric,
                window_start,
                units,
                _expires_at,
                _overage_units,
                _overage_cents,
            )) => {
                sqlx::query(
                    "UPDATE quota_budget \
                     SET used_units = used_units + $5, \
                         held_units = GREATEST(held_units - $5, 0) \
                     WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                       AND window_start = $4",
                )
                .bind(tenant_id)
                .bind(&provider_id)
                .bind(&metric)
                .bind(window_start)
                .bind(units)
                .execute(&mut *tx)
                .await
                .map_err(backend_error)?;

                tx.commit().await.map_err(backend_error)?;
                Ok(())
            }
            None => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM quota_reservation WHERE reservation_id = $1)",
                )
                .bind(id.as_uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(backend_error)?;
                tx.rollback().await.map_err(backend_error)?;
                if exists {
                    Ok(())
                } else {
                    Err(QuotaError::ReservationNotFound(id.to_string()))
                }
            }
        }
    }

    async fn release(&self, id: ReservationId) -> Result<(), QuotaError> {
        let mut tx = self.pool.begin().await.map_err(backend_error)?;
        let row: Option<ReservationRow> = sqlx::query_as(
            "UPDATE quota_reservation SET settled_at = now() \
             WHERE reservation_id = $1 AND settled_at IS NULL \
             RETURNING reservation_id, tenant_id, provider_id, metric, window_start, \
                       units, expires_at, overage_units, overage_cents",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(backend_error)?;

        match row {
            Some((
                _reservation_id,
                tenant_id,
                provider_id,
                metric,
                window_start,
                units,
                _expires_at,
                overage_units,
                overage_cents,
            )) => {
                sqlx::query(
                    "UPDATE quota_budget \
                     SET held_units = GREATEST(held_units - $5, 0), \
                         overage_units = GREATEST(overage_units - $6, 0), \
                         overage_cents = GREATEST(overage_cents - $7, 0) \
                     WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 \
                       AND window_start = $4",
                )
                .bind(tenant_id)
                .bind(&provider_id)
                .bind(&metric)
                .bind(window_start)
                .bind(units)
                .bind(overage_units)
                .bind(overage_cents)
                .execute(&mut *tx)
                .await
                .map_err(backend_error)?;

                tx.commit().await.map_err(backend_error)?;
                Ok(())
            }
            None => {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM quota_reservation WHERE reservation_id = $1)",
                )
                .bind(id.as_uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(backend_error)?;
                tx.rollback().await.map_err(backend_error)?;
                if exists {
                    Ok(())
                } else {
                    Err(QuotaError::ReservationNotFound(id.to_string()))
                }
            }
        }
    }

    async fn usage(&self, key: &QuotaKey) -> Result<Usage, QuotaError> {
        let row: Option<UsageRow> = sqlx::query_as(
            "SELECT limit_units, used_units, held_units, overage_units, overage_cents, window_end \
             FROM quota_budget \
             WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 AND window_end > $4 \
             ORDER BY window_start DESC LIMIT 1",
        )
        .bind(*key.tenant.as_uuid())
        .bind(key.provider.as_str())
        .bind(key.metric.as_str())
        .bind(Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend_error)?;

        let (limit_units, used_units, held_units, overage_units, overage_cents, window_end) = row
            .ok_or_else(
            || QuotaError::UnknownBudget(format!("{}/{}", key.tenant, key.provider)),
        )?;

        Ok(Usage {
            limit: to_u64(limit_units),
            used: to_u64(used_units),
            held: to_u64(held_units),
            overage: to_u64(overage_units),
            overage_cents: to_u64(overage_cents),
            reset_at: window_end,
        })
    }
}

async fn insert_reservation(
    tx: &mut Transaction<'_, Postgres>,
    reservation_id: Uuid,
    request: &ReserveRequest,
    window_start: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    overage_units: i64,
    overage_cents: i64,
) -> Result<bool, QuotaError> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO quota_reservation \
           (reservation_id, tenant_id, provider_id, metric, window_start, units, \
            idempotency_key, expires_at, policy, overage_units, overage_cents) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         ON CONFLICT DO NOTHING \
         RETURNING reservation_id",
    )
    .bind(reservation_id)
    .bind(*request.budget.tenant.as_uuid())
    .bind(request.budget.provider.as_str())
    .bind(request.budget.metric.as_str())
    .bind(window_start)
    .bind(to_i64(request.units, "units")?)
    .bind(request.idempotency_key.as_deref())
    .bind(expires_at)
    .bind(policy_name(request.budget.policy))
    .bind(overage_units)
    .bind(overage_cents)
    .fetch_optional(&mut **tx)
    .await
    .map_err(backend_error)?;

    Ok(inserted.is_some())
}

async fn resolve_conflict(
    tx: &mut Transaction<'_, Postgres>,
    request: &ReserveRequest,
    units: i64,
) -> Result<ReserveOutcome, QuotaError> {
    let Some(idempotency_key) = request.idempotency_key.as_deref() else {
        return Err(QuotaError::Backend(
            "reservation identifier collision".to_owned(),
        ));
    };

    let row: ReservationRow = sqlx::query_as(
        "SELECT reservation_id, tenant_id, provider_id, metric, window_start, units, \
                expires_at, overage_units, overage_cents \
         FROM quota_reservation \
         WHERE tenant_id = $1 AND provider_id = $2 AND metric = $3 AND idempotency_key = $4",
    )
    .bind(*request.budget.tenant.as_uuid())
    .bind(request.budget.provider.as_str())
    .bind(request.budget.metric.as_str())
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await
    .map_err(backend_error)?;

    if row.5 != units {
        return Err(QuotaError::IdempotencyConflict(format!(
            "idempotency key `{idempotency_key}` was first used for {} units, not {}",
            row.5, request.units
        )));
    }

    Ok(outcome_from_row(
        row,
        &request.budget,
        Some(idempotency_key.to_owned()),
    ))
}

fn outcome_from_row(
    row: ReservationRow,
    budget: &Budget,
    idempotency_key: Option<String>,
) -> ReserveOutcome {
    let (
        reservation_id,
        _tenant_id,
        _provider_id,
        _metric,
        window_start,
        units,
        expires_at,
        overage_units,
        overage_cents,
    ) = row;

    let reservation = Reservation {
        id: ReservationId::from_uuid(reservation_id),
        key: QuotaKey::new(budget.tenant, budget.provider, budget.metric, window_start),
        units: to_u64(units),
        idempotency_key,
        expires_at,
        policy: budget.policy,
    };

    if overage_units > 0 {
        ReserveOutcome::Overage {
            reservation,
            cost_cents: to_u64(overage_cents),
        }
    } else {
        ReserveOutcome::Reserved(reservation)
    }
}

fn parse_options(database_url: &str) -> Result<PgConnectOptions, QuotaError> {
    PgConnectOptions::from_str(database_url)
        .map_err(|_| QuotaError::Config("the Postgres connection string is invalid".to_owned()))
}

fn policy_name(policy: ExhaustionPolicy) -> &'static str {
    match policy {
        ExhaustionPolicy::HardStop => "hard_stop",
        ExhaustionPolicy::Overage { .. } => "overage",
        ExhaustionPolicy::Degrade => "degrade",
        ExhaustionPolicy::Queue => "queue",
    }
}

fn policy_unit_cost(policy: ExhaustionPolicy) -> Option<i64> {
    match policy {
        ExhaustionPolicy::Overage { unit_cost_cents } => Some(i64::from(unit_cost_cents)),
        ExhaustionPolicy::HardStop | ExhaustionPolicy::Degrade | ExhaustionPolicy::Queue => None,
    }
}

fn to_i64(value: u64, field: &str) -> Result<i64, QuotaError> {
    i64::try_from(value)
        .map_err(|_| QuotaError::Config(format!("{field} exceeds the Postgres BIGINT range")))
}

fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn backend_error(error: sqlx::Error) -> QuotaError {
    if let sqlx::Error::Database(database) = &error {
        if let Some(code) = database.code() {
            return QuotaError::Backend(format!("database error {code}"));
        }
    }
    QuotaError::Backend("database operation failed".to_owned())
}
