//! Redis-backed quota storage.
//!
//! [`RedisQuotaStore`] implements [`QuotaStore`] over a caller-supplied
//! [`ConnectionManager`]. The caller owns the connection, its TLS settings, and
//! its credentials; this crate reads no environment variable, no URL from the
//! environment, and no credential file, and it never disables TLS.
//!
//! The reserve claim is one Lua script (`reserve.lua`) executed with
//! `EVALSHA`, so the capacity check, the hold write, and the counters are atomic
//! on the server and two pods cannot both take the final unit. Settlement runs a
//! second script (`settle.lua`) that marks the reservation settled before
//! touching the budget, so a repeated commit or release is a no-op.
//!
//! # Key schema
//!
//! - `quota:budget:{tenant}:{provider}:{metric}:{window_start_ms}` â€” a hash with
//!   `limit`, `used`, `held`, `overage_units`, `overage_cents`, `window_start`,
//!   and `window_end`.
//! - `quota:res:{tenant}:{reservation_id}` â€” a hash with `units`, `policy`,
//!   `unit_cost`, `expires`, `budget`, `overage_cents`, and `settled`.
//! - `quota:residx:{reservation_id}` â€” the reservation's key, so `commit` and
//!   `release` can settle by id (the 0.1 trait signature) without losing the
//!   tenant in the data key.
//! - `quota:idem:{tenant}:{provider}:{metric}:{window_start_ms}:{key}` â€” the
//!   reservation id an idempotency key already produced.
//! - `quota:index:{tenant}:{provider}:{metric}` â€” the window starts this bucket
//!   has used.
//!
//! Every budget, reservation, idempotency, and index key embeds the tenant.
//! Provider, metric, and caller keys are percent-escaped, so a `:` inside one
//! cannot forge another key segment.
//!
//! # Non-goals
//!
//! - No Redis cluster or sentinel configuration; the caller configures the client.
//! - No migration of existing Postgres rows.
//! - No change to the [`QuotaStore`] trait or the 0.1 value types.

mod keys;

use std::collections::HashMap;
use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use redis::{AsyncCommands, Script};

use crate::types::{
    ExhaustionPolicy, QuotaExceeded, QuotaKey, Reservation, ReservationId, ReserveOutcome,
    ReserveRequest, Usage,
};
use crate::{QuotaError, QuotaStore};

const RESERVE: &str = include_str!("reserve.lua");
const SETTLE: &str = include_str!("settle.lua");

/// A quota store backed by Redis.
///
/// Clones are cheap: the underlying [`ConnectionManager`] is shared.
#[derive(Clone)]
pub struct RedisQuotaStore {
    connection: ConnectionManager,
}

impl RedisQuotaStore {
    /// Wraps an existing connection manager.
    ///
    /// The caller is responsible for the connection's TLS and authentication
    /// settings.
    pub fn from_manager(connection: ConnectionManager) -> Self {
        Self { connection }
    }

    /// Connects to Redis using a caller-supplied URL.
    ///
    /// The URL is parsed but never stored or rendered. Use `rediss://` (or a
    /// TLS-enabled client) in production; this crate does not disable TLS.
    ///
    /// # Errors
    ///
    /// Returns [`QuotaError::Backend`] when the client cannot be created or the
    /// connection cannot be established, and [`QuotaError::Config`] when the URL
    /// is not a Redis URL.
    pub async fn connect(url: &str) -> Result<Self, QuotaError> {
        let client = redis::Client::open(url).map_err(config_error)?;
        let connection = ConnectionManager::new(client)
            .await
            .map_err(backend_error)?;
        Ok(Self { connection })
    }
}

impl fmt::Debug for RedisQuotaStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedisQuotaStore")
            .field("connection", &"[REDACTED]")
            .finish()
    }
}

#[async_trait]
impl QuotaStore for RedisQuotaStore {
    async fn reserve(&self, request: &ReserveRequest) -> Result<ReserveOutcome, QuotaError> {
        if request.units == 0 {
            return Err(QuotaError::Config(
                "a reservation must hold at least one unit".to_owned(),
            ));
        }
        if request.budget.metric.as_str().is_empty() {
            return Err(QuotaError::Config(
                "the metric must not be empty".to_owned(),
            ));
        }
        if request.budget.provider.as_str().is_empty() {
            return Err(QuotaError::Config(
                "the provider must not be empty".to_owned(),
            ));
        }
        let ttl_seconds = request.ttl.num_seconds();
        if ttl_seconds < 1 {
            return Err(QuotaError::Config(
                "the reservation TTL must be at least one second".to_owned(),
            ));
        }

        let budget = &request.budget;
        let now = Utc::now();
        let (window_start, window_end) = keys::window_bounds(budget.window, now);
        if window_end <= window_start {
            return Err(QuotaError::Config(
                "the window ends before it starts".to_owned(),
            ));
        }

        let key = QuotaKey::new(budget.tenant, budget.provider, budget.metric, window_start);
        let id = ReservationId::generate();
        let budget_key = keys::budget_key(budget.tenant, &budget.provider, budget.metric);
        let reservation_key = keys::reservation_key(budget.tenant, id);
        let index_key = keys::reservation_index_key(id);
        let bucket_index = keys::index_key(budget.tenant, &budget.provider, budget.metric);
        let caller_key = request.idempotency_key.as_deref().unwrap_or_default();
        let idem_key = if caller_key.is_empty() {
            budget_key.clone()
        } else {
            keys::idempotency_key(budget.tenant, &budget.provider, budget.metric, caller_key)
        };
        let (policy_tag, unit_cost) = policy_tag(budget.policy);
        let budget_ttl = (window_end - now).num_seconds().max(60) + 3600;

        let mut connection = self.connection.clone();
        let reply: Vec<String> = Script::new(RESERVE)
            .key(budget_key)
            .key(&reservation_key)
            .key(index_key)
            .key(idem_key)
            .key(bucket_index)
            .arg(id.to_string())
            .arg(request.units)
            .arg(budget.limit)
            .arg(ttl_seconds)
            .arg(policy_tag)
            .arg(unit_cost)
            .arg(window_end.timestamp_millis())
            .arg(window_start.timestamp_millis())
            .arg(budget_ttl)
            .arg(caller_key)
            .arg(keys::reservation_prefix())
            .arg(now.timestamp_millis())
            .invoke_async(&mut connection)
            .await
            .map_err(backend_error)?;

        self.explain_reply(&reply, key, request, id, window_end)
    }

    async fn commit(&self, id: ReservationId) -> Result<(), QuotaError> {
        self.settle(id, "commit").await
    }

    async fn release(&self, id: ReservationId) -> Result<(), QuotaError> {
        self.settle(id, "release").await
    }

    async fn usage(&self, key: &QuotaKey) -> Result<Usage, QuotaError> {
        let mut connection = self.connection.clone();
        let fields: HashMap<String, String> = connection
            .hgetall(keys::usage_key(key))
            .await
            .map_err(backend_error)?;

        let limit = parse_field(&fields, "limit")?;
        let used = parse_field(&fields, "used")?;
        let held = parse_field(&fields, "held")?;
        let overage = parse_field(&fields, "overage_units")?;
        let overage_cents = parse_field(&fields, "overage_cents")?;
        let window_end = parse_field(&fields, "window_end")?;
        let window_end = i64::try_from(window_end)
            .map_err(|_| QuotaError::Backend("the stored window end is out of range".to_owned()))?;
        let reset_at = DateTime::from_timestamp_millis(window_end).ok_or_else(|| {
            QuotaError::Backend("the stored window end is not a valid instant".to_owned())
        })?;

        Ok(Usage {
            limit,
            used,
            held,
            overage,
            overage_cents,
            reset_at,
        })
    }
}

impl RedisQuotaStore {
    async fn settle(&self, id: ReservationId, mode: &'static str) -> Result<(), QuotaError> {
        let index_key = keys::reservation_index_key(id);
        let mut connection = self.connection.clone();
        let reply: Vec<String> = Script::new(SETTLE)
            .key(index_key)
            .arg(mode)
            .invoke_async(&mut connection)
            .await
            .map_err(backend_error)?;

        match reply.first().map(String::as_str) {
            Some("OK" | "ALREADY") => Ok(()),
            Some("MISSING") => Err(QuotaError::ReservationNotFound(id.to_string())),
            other => Err(QuotaError::Backend(format!(
                "unexpected settlement reply: {}",
                other.unwrap_or("<empty>")
            ))),
        }
    }

    fn explain_reply(
        &self,
        reply: &[String],
        key: QuotaKey,
        request: &ReserveRequest,
        id: ReservationId,
        reset_at: DateTime<Utc>,
    ) -> Result<ReserveOutcome, QuotaError> {
        let status = reply.first().map(String::as_str).unwrap_or_default();
        match status {
            "OK" => Ok(ReserveOutcome::Reserved(Reservation {
                id,
                key,
                units: request.units,
                idempotency_key: request.idempotency_key.clone(),
                expires_at: Utc::now() + request.ttl,
                policy: request.budget.policy,
            })),
            "OV" => {
                let cost_cents = parse_u64(reply.get(1), "overage cost")?;
                Ok(ReserveOutcome::Overage {
                    reservation: Reservation {
                        id,
                        key,
                        units: request.units,
                        idempotency_key: request.idempotency_key.clone(),
                        expires_at: Utc::now() + request.ttl,
                        policy: request.budget.policy,
                    },
                    cost_cents,
                })
            }
            "DENY" => {
                let remaining = parse_u64(reply.get(1), "remaining units")?;
                Ok(ReserveOutcome::Denied(QuotaExceeded {
                    requested: request.units,
                    remaining,
                    reset_at,
                    upgrade_hint: "request a higher quota for this provider",
                }))
            }
            "DEGRADE" => Ok(ReserveOutcome::Degraded { reset_at }),
            "QUEUE" => Ok(ReserveOutcome::Queued { reset_at }),
            "ALREADY" => {
                let stored_id = reply.get(1).ok_or_else(|| malformed("reservation id"))?;
                let units = parse_u64(reply.get(2), "units")?;
                let expires = parse_u64(reply.get(3), "expiry")?;
                let policy = policy_from(
                    reply.get(4).map(String::as_str).unwrap_or("hard_stop"),
                    parse_u64(reply.get(5), "unit cost")?,
                );
                let id = uuid::Uuid::parse_str(stored_id)
                    .map(ReservationId::from_uuid)
                    .map_err(|_| malformed("reservation id"))?;
                let expires_at =
                    DateTime::from_timestamp_millis(i64::try_from(expires).unwrap_or(0))
                        .ok_or_else(|| malformed("expiry"))?;
                Ok(ReserveOutcome::Reserved(Reservation {
                    id,
                    key,
                    units,
                    idempotency_key: request.idempotency_key.clone(),
                    expires_at,
                    policy,
                }))
            }
            "CONFLICT" => Err(QuotaError::IdempotencyConflict(format!(
                "the idempotency key was reused with a different unit count for {}/{}/{}",
                key.tenant, key.provider, key.metric
            ))),
            "CONFIG" => Err(QuotaError::Config(
                reply
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "invalid request".to_owned()),
            )),
            other => Err(QuotaError::Backend(format!(
                "unexpected reserve reply: {other}"
            ))),
        }
    }
}

fn policy_tag(policy: ExhaustionPolicy) -> (&'static str, u64) {
    match policy {
        ExhaustionPolicy::HardStop => ("hard_stop", 0),
        ExhaustionPolicy::Overage { unit_cost_cents } => ("overage", u64::from(unit_cost_cents)),
        ExhaustionPolicy::Degrade => ("degrade", 0),
        ExhaustionPolicy::Queue => ("queue", 0),
    }
}

fn policy_from(tag: &str, unit_cost_cents: u64) -> ExhaustionPolicy {
    match tag {
        "overage" => ExhaustionPolicy::Overage {
            unit_cost_cents: u32::try_from(unit_cost_cents).unwrap_or(u32::MAX),
        },
        "degrade" => ExhaustionPolicy::Degrade,
        "queue" => ExhaustionPolicy::Queue,
        _ => ExhaustionPolicy::HardStop,
    }
}

fn parse_field(fields: &HashMap<String, String>, name: &str) -> Result<u64, QuotaError> {
    fields
        .get(name)
        .ok_or_else(|| QuotaError::UnknownBudget(format!("the ledger has no `{name}` field")))?
        .parse::<u64>()
        .map_err(|_| malformed(name))
}

fn parse_u64(value: Option<&String>, name: &str) -> Result<u64, QuotaError> {
    value
        .ok_or_else(|| malformed(name))?
        .parse::<u64>()
        .map_err(|_| malformed(name))
}

fn malformed(name: &str) -> QuotaError {
    QuotaError::Backend(format!("the backend reply has a malformed `{name}` value"))
}

fn config_error(error: redis::RedisError) -> QuotaError {
    QuotaError::Config(redact(&error.to_string()))
}

fn backend_error(error: redis::RedisError) -> QuotaError {
    QuotaError::Backend(redact(&error.to_string()))
}

/// Removes anything that could be a connection string or a credential.
fn redact(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for token in message.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let lower = token.to_ascii_lowercase();
        if token.contains("://") || lower.contains("password") || lower.contains("secret") {
            out.push_str("[REDACTED]");
        } else {
            out.push_str(token);
        }
    }
    out.chars().take(512).collect()
}
