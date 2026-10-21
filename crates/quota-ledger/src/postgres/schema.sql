-- quota-ledger Postgres schema (0.1).
--
-- Apply this file with the application's migration tool. The
-- `PostgresQuotaStore::apply_schema` helper executes it for local development
-- and tests; it is not a migration framework and does not track versions.
--
-- Units are BIGINT and costs are in cents. All timestamps are UTC.

CREATE TABLE IF NOT EXISTS quota_budget (
    tenant_id UUID NOT NULL,
    provider_id TEXT NOT NULL,
    metric TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    limit_units BIGINT NOT NULL CHECK (limit_units >= 0),
    policy TEXT NOT NULL CHECK (policy IN ('hard_stop', 'overage', 'degrade', 'queue')),
    unit_cost_cents BIGINT,
    used_units BIGINT NOT NULL DEFAULT 0 CHECK (used_units >= 0),
    held_units BIGINT NOT NULL DEFAULT 0 CHECK (held_units >= 0),
    overage_units BIGINT NOT NULL DEFAULT 0 CHECK (overage_units >= 0),
    overage_cents BIGINT NOT NULL DEFAULT 0 CHECK (overage_cents >= 0),
    PRIMARY KEY (tenant_id, provider_id, metric, window_start)
);

CREATE INDEX IF NOT EXISTS quota_budget_active_idx
    ON quota_budget (tenant_id, provider_id, metric, window_end);

CREATE TABLE IF NOT EXISTS quota_reservation (
    reservation_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    provider_id TEXT NOT NULL,
    metric TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    units BIGINT NOT NULL CHECK (units > 0),
    idempotency_key TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    policy TEXT NOT NULL,
    overage_units BIGINT NOT NULL DEFAULT 0 CHECK (overage_units >= 0),
    overage_cents BIGINT NOT NULL DEFAULT 0 CHECK (overage_cents >= 0),
    settled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (tenant_id, provider_id, metric, window_start)
        REFERENCES quota_budget (tenant_id, provider_id, metric, window_start)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS quota_reservation_idempotency_idx
    ON quota_reservation (tenant_id, provider_id, metric, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS quota_reservation_expiry_idx
    ON quota_reservation (expires_at)
    WHERE settled_at IS NULL;

CREATE INDEX IF NOT EXISTS quota_reservation_usage_idx
    ON quota_reservation (tenant_id, provider_id, metric, window_start);

CREATE TABLE IF NOT EXISTS quota_dead_hold (
    reservation_id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    provider_id TEXT NOT NULL,
    metric TEXT NOT NULL,
    window_start TIMESTAMPTZ NOT NULL,
    units BIGINT NOT NULL CHECK (units > 0),
    overage_units BIGINT NOT NULL DEFAULT 0 CHECK (overage_units >= 0),
    overage_cents BIGINT NOT NULL DEFAULT 0 CHECK (overage_cents >= 0),
    reclaimed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS quota_dead_hold_reclaimed_idx
    ON quota_dead_hold (reclaimed_at);
