use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Trade
// ---------------------------------------------------------------------------

/// A fully executed trade recorded in the `trades` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Trade {
    pub id: Uuid,
    pub strategy: String,
    pub exchange: String,
    pub symbol: String,
    /// "buy" or "sell"
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    /// Realised profit expressed in USDT cents (positive = profit, negative = loss).
    pub profit_cents: i64,
    pub executed_at: DateTime<Utc>,
    pub metadata: JsonValue,
}

/// Data required to insert a new trade row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTrade {
    pub strategy: String,
    pub exchange: String,
    pub symbol: String,
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub profit_cents: i64,
    pub executed_at: DateTime<Utc>,
    pub metadata: JsonValue,
}

// ---------------------------------------------------------------------------
// Liquidation
// ---------------------------------------------------------------------------

/// A liquidation attempt recorded in the `liquidations` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Liquidation {
    pub id: Uuid,
    pub borrower: String,
    pub protocol: String,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub debt_amount_wei: Decimal,
    pub collateral_received_wei: Decimal,
    pub profit_cents: i64,
    /// "pending" | "success" | "failed"
    pub status: String,
    pub tx_hash: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub error: Option<String>,
}

/// Data required to insert a new liquidation row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLiquidation {
    pub borrower: String,
    pub protocol: String,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub debt_amount_wei: Decimal,
    pub collateral_received_wei: Decimal,
    pub profit_cents: i64,
    pub status: String,
    pub tx_hash: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// TokenUnlock
// ---------------------------------------------------------------------------

/// A tracked token unlock event recorded in the `token_unlocks` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TokenUnlock {
    pub id: Uuid,
    pub token_address: String,
    pub symbol: String,
    pub unlock_at: DateTime<Utc>,
    pub amount_wei: Decimal,
    /// Estimated USD value in cents.
    pub usd_value_cents: i64,
    /// "Team" | "Investor" | "Ecosystem" | "Unknown"
    pub beneficiary_type: String,
    /// Optional foreign key to a related trade.
    pub trade_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Data required to insert a new token_unlock row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTokenUnlock {
    pub token_address: String,
    pub symbol: String,
    pub unlock_at: DateTime<Utc>,
    pub amount_wei: Decimal,
    pub usd_value_cents: i64,
    pub beneficiary_type: String,
    pub trade_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// BorrowerPosition
// ---------------------------------------------------------------------------

/// A snapshot of an on-chain borrower position recorded in `borrower_positions`.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BorrowerPositionRow {
    pub id: Uuid,
    pub address: String,
    pub protocol: String,
    /// Health factor expressed as a NUMERIC (1e18 = 1.0, matching the Wei scale).
    pub health_factor: Decimal,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub total_collateral_wei: Decimal,
    pub total_debt_wei: Decimal,
    pub updated_at: DateTime<Utc>,
}

/// Data required to upsert a borrower position row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewBorrowerPosition {
    pub address: String,
    pub protocol: String,
    pub health_factor: Decimal,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub total_collateral_wei: Decimal,
    pub total_debt_wei: Decimal,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// SystemEvent
// ---------------------------------------------------------------------------

/// A generic system event recorded in the `system_events` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SystemEvent {
    pub id: Uuid,
    pub event_type: String,
    pub payload: JsonValue,
    pub occurred_at: DateTime<Utc>,
}
