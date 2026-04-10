use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Price in quote currency, stored as Decimal to avoid float precision loss.
/// Example: BTC/USDT ask = 80_000.50 USDT
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Price(pub Decimal);

/// Amount of base asset, stored as Decimal.
/// Example: 0.5 BTC
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Amount(pub Decimal);

/// Amount in wei (18 decimal EVM representation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Wei(pub u128);

impl Wei {
    pub const ZERO: Wei = Wei(0);
    pub const ONE_ETH: Wei = Wei(1_000_000_000_000_000_000);

    pub fn to_decimal(self) -> Decimal {
        Decimal::new(self.0 as i64, 18)
    }
}

/// Trading pair symbol, e.g. "BTCUSDT"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Self {
        Symbol(s.into())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Supported CEX exchanges
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    Binance,
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Exchange::Binance => write!(f, "Binance"),
        }
    }
}

/// Supported DeFi lending protocols
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LendingProtocol {
    AaveV3,
    CompoundV3,
    Morpho,
}

impl fmt::Display for LendingProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LendingProtocol::AaveV3 => write!(f, "Aave V3"),
            LendingProtocol::CompoundV3 => write!(f, "Compound V3"),
            LendingProtocol::Morpho => write!(f, "Morpho"),
        }
    }
}

/// Best bid/ask snapshot from an exchange order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub bid: Price,
    pub bid_qty: Amount,
    pub ask: Price,
    pub ask_qty: Amount,
    pub timestamp_ms: u64,
}

/// On-chain borrower position on a lending protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorrowerPosition {
    pub address: String,
    pub protocol: LendingProtocol,
    /// Health factor in wei units (1e18 = 1.0). Below 1e18 = liquidatable.
    pub health_factor: Wei,
    pub collateral_asset: String,
    pub debt_asset: String,
    pub total_collateral_wei: Wei,
    pub total_debt_wei: Wei,
    pub liquidation_threshold_bps: u32,
}

impl BorrowerPosition {
    /// Returns true if this position can be liquidated
    pub fn is_liquidatable(&self) -> bool {
        self.health_factor < Wei::ONE_ETH
    }

    /// Maximum debt that can be covered in one liquidation call (50% of total debt)
    pub fn max_liquidatable_debt(&self) -> Wei {
        Wei(self.total_debt_wei.0 / 2)
    }
}

/// A detected liquidation opportunity ready for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationOpportunity {
    pub borrower: String,
    pub protocol: LendingProtocol,
    pub collateral_asset: String,
    pub collateral_symbol: Symbol,
    pub debt_asset: String,
    pub debt_amount_wei: Wei,
    pub health_factor: Wei,
    /// Liquidation bonus in basis points (e.g. 500 = 5%)
    pub bonus_bps: u32,
    /// Estimated profit in USDT cents
    pub estimated_profit_cents: i64,
}

/// A token unlock event parsed from a vesting contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUnlockEvent {
    pub token_address: String,
    pub symbol: Symbol,
    pub unlock_timestamp: i64,
    pub unlock_amount_wei: Wei,
    /// Estimated USD value in cents
    pub estimated_usd_cents: i64,
    /// Type of beneficiary (affects bearishness)
    pub beneficiary_type: BeneficiaryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeneficiaryType {
    /// Team / founders - most bearish
    Team,
    /// Investors / VCs - bearish
    Investor,
    /// Ecosystem / community fund - neutral/slightly bullish
    Ecosystem,
    /// Unknown
    Unknown,
}
