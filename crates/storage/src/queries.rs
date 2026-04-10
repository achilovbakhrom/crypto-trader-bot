use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::error::StorageError;
use crate::models::{
    BorrowerPositionRow, Liquidation, NewBorrowerPosition, NewLiquidation, NewTokenUnlock,
    NewTrade, SystemEvent, TokenUnlock, Trade,
};

// ── trades ────────────────────────────────────────────────────────────────────

#[instrument(skip(pool, trade), fields(strategy = %trade.strategy, symbol = %trade.symbol))]
pub async fn insert_trade(pool: &PgPool, trade: NewTrade) -> Result<Uuid, StorageError> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO trades
            (strategy, exchange, symbol, side, quantity, price, fee,
             profit_cents, executed_at, metadata)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(trade.strategy)
    .bind(trade.exchange)
    .bind(trade.symbol)
    .bind(trade.side)
    .bind(trade.quantity)
    .bind(trade.price)
    .bind(trade.fee)
    .bind(trade.profit_cents)
    .bind(trade.executed_at)
    .bind(trade.metadata)
    .fetch_one(pool)
    .await?;

    debug!(%id, "trade inserted");
    Ok(id)
}

#[instrument(skip(pool))]
pub async fn get_recent_trades(pool: &PgPool, limit: i64) -> Result<Vec<Trade>, StorageError> {
    let rows = sqlx::query_as::<_, Trade>(
        r#"
        SELECT id, strategy, exchange, symbol, side,
               quantity, price, fee, profit_cents, executed_at, metadata
        FROM trades
        ORDER BY executed_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── liquidations ──────────────────────────────────────────────────────────────

#[instrument(skip(pool, liquidation), fields(borrower = %liquidation.borrower))]
pub async fn insert_liquidation(
    pool: &PgPool,
    liquidation: NewLiquidation,
) -> Result<Uuid, StorageError> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO liquidations
            (borrower, protocol, collateral_asset, debt_asset,
             debt_amount_wei, collateral_received_wei, profit_cents,
             status, tx_hash, executed_at, error)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#,
    )
    .bind(liquidation.borrower)
    .bind(liquidation.protocol)
    .bind(liquidation.collateral_asset)
    .bind(liquidation.debt_asset)
    .bind(liquidation.debt_amount_wei)
    .bind(liquidation.collateral_received_wei)
    .bind(liquidation.profit_cents)
    .bind(liquidation.status)
    .bind(liquidation.tx_hash)
    .bind(liquidation.executed_at)
    .bind(liquidation.error)
    .fetch_one(pool)
    .await?;

    debug!(%id, "liquidation inserted");
    Ok(id)
}

#[instrument(skip(pool), fields(%id, %status))]
pub async fn update_liquidation_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    tx_hash: Option<&str>,
    error: Option<&str>,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        r#"
        UPDATE liquidations
        SET status = $2, tx_hash = $3, error = $4
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(tx_hash)
    .bind(error)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(StorageError::NotFound);
    }

    debug!(%id, %status, "liquidation status updated");
    Ok(())
}

#[instrument(skip(pool))]
pub async fn get_liquidations(pool: &PgPool, limit: i64) -> Result<Vec<Liquidation>, StorageError> {
    let rows = sqlx::query_as::<_, Liquidation>(
        r#"
        SELECT id, borrower, protocol, collateral_asset, debt_asset,
               debt_amount_wei, collateral_received_wei, profit_cents,
               status, tx_hash, executed_at, error
        FROM liquidations
        ORDER BY executed_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── token_unlocks ─────────────────────────────────────────────────────────────

#[instrument(skip(pool, unlock), fields(symbol = %unlock.symbol))]
pub async fn insert_token_unlock(
    pool: &PgPool,
    unlock: NewTokenUnlock,
) -> Result<Uuid, StorageError> {
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO token_unlocks
            (token_address, symbol, unlock_at, amount_wei,
             usd_value_cents, beneficiary_type, trade_id, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(unlock.token_address)
    .bind(unlock.symbol)
    .bind(unlock.unlock_at)
    .bind(unlock.amount_wei)
    .bind(unlock.usd_value_cents)
    .bind(unlock.beneficiary_type)
    .bind(unlock.trade_id)
    .bind(unlock.created_at)
    .fetch_one(pool)
    .await?;

    debug!(%id, "token unlock inserted");
    Ok(id)
}

pub async fn get_token_unlocks_by_address(
    pool: &PgPool,
    token_address: &str,
) -> Result<Vec<TokenUnlock>, StorageError> {
    let rows = sqlx::query_as::<_, TokenUnlock>(
        r#"
        SELECT id, token_address, symbol, unlock_at, amount_wei,
               usd_value_cents, beneficiary_type, trade_id, created_at
        FROM token_unlocks
        WHERE token_address = $1
        ORDER BY unlock_at ASC
        "#,
    )
    .bind(token_address)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── borrower_positions ────────────────────────────────────────────────────────

#[instrument(skip(pool, position), fields(address = %position.address))]
pub async fn upsert_borrower_position(
    pool: &PgPool,
    position: NewBorrowerPosition,
) -> Result<(), StorageError> {
    sqlx::query(
        r#"
        INSERT INTO borrower_positions
            (address, protocol, health_factor, collateral_asset, debt_asset,
             total_collateral_wei, total_debt_wei, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (address, protocol)
        DO UPDATE SET
            health_factor        = EXCLUDED.health_factor,
            collateral_asset     = EXCLUDED.collateral_asset,
            debt_asset           = EXCLUDED.debt_asset,
            total_collateral_wei = EXCLUDED.total_collateral_wei,
            total_debt_wei       = EXCLUDED.total_debt_wei,
            updated_at           = EXCLUDED.updated_at
        "#,
    )
    .bind(position.address)
    .bind(position.protocol)
    .bind(position.health_factor)
    .bind(position.collateral_asset)
    .bind(position.debt_asset)
    .bind(position.total_collateral_wei)
    .bind(position.total_debt_wei)
    .bind(position.updated_at)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_liquidatable_positions(
    pool: &PgPool,
    threshold: Decimal,
) -> Result<Vec<BorrowerPositionRow>, StorageError> {
    let rows = sqlx::query_as::<_, BorrowerPositionRow>(
        r#"
        SELECT id, address, protocol, health_factor, collateral_asset,
               debt_asset, total_collateral_wei, total_debt_wei, updated_at
        FROM borrower_positions
        WHERE health_factor < $1
        ORDER BY health_factor ASC
        "#,
    )
    .bind(threshold)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

// ── system_events ─────────────────────────────────────────────────────────────

#[instrument(skip(pool, payload), fields(%event_type))]
pub async fn log_event(
    pool: &PgPool,
    event_type: &str,
    payload: impl serde::Serialize + Send + Sync,
) -> Result<(), StorageError> {
    let occurred_at: DateTime<Utc> = Utc::now();
    let payload_val = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);

    sqlx::query(
        r#"
        INSERT INTO system_events (event_type, payload, occurred_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(event_type)
    .bind(payload_val)
    .bind(occurred_at)
    .execute(pool)
    .await?;

    debug!(%event_type, "system event logged");
    Ok(())
}

pub async fn get_all_recent_events(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<SystemEvent>, StorageError> {
    let rows = sqlx::query_as::<_, SystemEvent>(
        r#"
        SELECT id, event_type, payload, occurred_at
        FROM system_events
        ORDER BY occurred_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
