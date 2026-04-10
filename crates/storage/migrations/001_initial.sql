-- 001_initial.sql
-- Initial schema for the crypto trading bot storage layer.

-- -----------------------------------------------------------------------
-- trades
-- Records every executed trade across all strategies / exchanges.
-- -----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS trades (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    strategy       TEXT        NOT NULL,
    exchange       TEXT        NOT NULL,
    symbol         TEXT        NOT NULL,
    side           TEXT        NOT NULL,          -- 'buy' | 'sell'
    quantity       NUMERIC     NOT NULL,
    price          NUMERIC     NOT NULL,
    fee            NUMERIC     NOT NULL DEFAULT 0,
    profit_cents   BIGINT      NOT NULL DEFAULT 0,
    executed_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata       JSONB       NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS trades_strategy_idx      ON trades (strategy);
CREATE INDEX IF NOT EXISTS trades_exchange_idx      ON trades (exchange);
CREATE INDEX IF NOT EXISTS trades_symbol_idx        ON trades (symbol);
CREATE INDEX IF NOT EXISTS trades_executed_at_idx   ON trades (executed_at DESC);

-- -----------------------------------------------------------------------
-- liquidations
-- Records every liquidation attempt (successful or failed).
-- -----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS liquidations (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    borrower                TEXT        NOT NULL,
    protocol                TEXT        NOT NULL,
    collateral_asset        TEXT        NOT NULL,
    debt_asset              TEXT        NOT NULL,
    debt_amount_wei         NUMERIC     NOT NULL,
    collateral_received_wei NUMERIC     NOT NULL DEFAULT 0,
    profit_cents            BIGINT      NOT NULL DEFAULT 0,
    status                  TEXT        NOT NULL,  -- 'pending' | 'success' | 'failed'
    tx_hash                 TEXT,
    executed_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    error                   TEXT
);

CREATE INDEX IF NOT EXISTS liquidations_borrower_idx     ON liquidations (borrower);
CREATE INDEX IF NOT EXISTS liquidations_protocol_idx     ON liquidations (protocol);
CREATE INDEX IF NOT EXISTS liquidations_status_idx       ON liquidations (status);
CREATE INDEX IF NOT EXISTS liquidations_executed_at_idx  ON liquidations (executed_at DESC);

-- -----------------------------------------------------------------------
-- token_unlocks
-- Tracked token unlock / vesting events.
-- -----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS token_unlocks (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    token_address    TEXT        NOT NULL,
    symbol           TEXT        NOT NULL,
    unlock_at        TIMESTAMPTZ NOT NULL,
    amount_wei       NUMERIC     NOT NULL,
    usd_value_cents  BIGINT      NOT NULL DEFAULT 0,
    beneficiary_type TEXT        NOT NULL,  -- 'Team' | 'Investor' | 'Ecosystem' | 'Unknown'
    trade_id         UUID        REFERENCES trades (id) ON DELETE SET NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS token_unlocks_token_address_idx ON token_unlocks (token_address);
CREATE INDEX IF NOT EXISTS token_unlocks_symbol_idx        ON token_unlocks (symbol);
CREATE INDEX IF NOT EXISTS token_unlocks_unlock_at_idx     ON token_unlocks (unlock_at ASC);
CREATE INDEX IF NOT EXISTS token_unlocks_trade_id_idx      ON token_unlocks (trade_id);

-- -----------------------------------------------------------------------
-- borrower_positions
-- Snapshot of every monitored on-chain lending position.
-- One row per (address, protocol) – upserted on every refresh.
-- -----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS borrower_positions (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    address              TEXT        NOT NULL,
    protocol             TEXT        NOT NULL,
    health_factor        NUMERIC     NOT NULL,
    collateral_asset     TEXT        NOT NULL,
    debt_asset           TEXT        NOT NULL,
    total_collateral_wei NUMERIC     NOT NULL,
    total_debt_wei       NUMERIC     NOT NULL,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (address, protocol)
);

CREATE INDEX IF NOT EXISTS borrower_positions_address_idx      ON borrower_positions (address);
CREATE INDEX IF NOT EXISTS borrower_positions_protocol_idx     ON borrower_positions (protocol);
CREATE INDEX IF NOT EXISTS borrower_positions_health_factor_idx ON borrower_positions (health_factor ASC);
CREATE INDEX IF NOT EXISTS borrower_positions_updated_at_idx   ON borrower_positions (updated_at DESC);

-- -----------------------------------------------------------------------
-- system_events
-- Generic event log for auditing / observability.
-- -----------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS system_events (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type   TEXT        NOT NULL,
    payload      JSONB       NOT NULL DEFAULT '{}',
    occurred_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS system_events_event_type_idx  ON system_events (event_type);
CREATE INDEX IF NOT EXISTS system_events_occurred_at_idx ON system_events (occurred_at DESC);
