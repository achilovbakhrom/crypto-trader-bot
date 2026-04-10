use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use tracing::{debug, info};

use trader_core::types::{BeneficiaryType, TokenUnlockEvent};
use execution::binance::BinanceClient;
use execution::order::OrderResult;

use crate::error::TokenUnlockError;

/// Decides whether to trade an unlock event and executes the trade.
pub struct UnlockTrader {
    binance: Arc<BinanceClient>,
    min_unlock_usd_cents: i64,
    days_before_entry: u32,
    days_before_exit: u32,
}

impl UnlockTrader {
    /// Create a new trader.
    ///
    /// * `binance`               – shared Binance client
    /// * `min_unlock_usd_cents`  – minimum USD value (cents) of an unlock event to trade
    /// * `days_before_entry`     – open short at most this many days before the unlock
    /// * `days_before_exit`      – close short when fewer than this many days remain
    pub fn new(
        binance: Arc<BinanceClient>,
        min_unlock_usd_cents: i64,
        days_before_entry: u32,
        days_before_exit: u32,
    ) -> Self {
        Self {
            binance,
            min_unlock_usd_cents,
            days_before_entry,
            days_before_exit,
        }
    }

    // -----------------------------------------------------------------------
    // Decision logic
    // -----------------------------------------------------------------------

    /// Returns `true` if this unlock event is worth trading.
    ///
    /// Criteria (all must hold):
    /// 1. `estimated_usd_cents >= min_unlock_usd_cents`
    /// 2. `beneficiary_type` is `Team` or `Investor` (not `Ecosystem` or `Unknown`)
    /// 3. Days until unlock is within `[days_before_exit, days_before_entry]`
    pub fn should_trade(&self, event: &TokenUnlockEvent, now: DateTime<Utc>) -> bool {
        // --- criterion 1: minimum size ---
        if event.estimated_usd_cents < self.min_unlock_usd_cents {
            debug!(
                symbol            = %event.symbol,
                usd_cents         = event.estimated_usd_cents,
                min_unlock_cents  = self.min_unlock_usd_cents,
                "skipping: unlock too small"
            );
            return false;
        }

        // --- criterion 2: beneficiary type ---
        match event.beneficiary_type {
            BeneficiaryType::Team | BeneficiaryType::Investor => {}
            BeneficiaryType::Ecosystem | BeneficiaryType::Unknown => {
                debug!(
                    symbol           = %event.symbol,
                    beneficiary_type = ?event.beneficiary_type,
                    "skipping: non-bearish beneficiary type"
                );
                return false;
            }
        }

        // --- criterion 3: timing window ---
        let now_ts = now.timestamp();
        let days_until_unlock = (event.unlock_timestamp - now_ts) / 86_400;

        if days_until_unlock < self.days_before_exit as i64
            || days_until_unlock > self.days_before_entry as i64
        {
            debug!(
                symbol            = %event.symbol,
                days_until_unlock,
                days_before_entry = self.days_before_entry,
                days_before_exit  = self.days_before_exit,
                "skipping: outside trading window"
            );
            return false;
        }

        info!(
            symbol            = %event.symbol,
            usd_cents         = event.estimated_usd_cents,
            beneficiary_type  = ?event.beneficiary_type,
            days_until_unlock,
            "unlock event qualifies for trade"
        );

        true
    }

    // -----------------------------------------------------------------------
    // Execution
    // -----------------------------------------------------------------------

    /// Open a short position by placing a market sell.
    ///
    /// `size_usd_cents` is divided by `current_price_cents` to derive quantity.
    /// Returns `TradeError` if the price is zero or the order fails.
    pub async fn open_short(
        &self,
        symbol: &str,
        size_usd_cents: i64,
        current_price_cents: i64,
    ) -> Result<OrderResult, TokenUnlockError> {
        if current_price_cents <= 0 {
            return Err(TokenUnlockError::TradeError(format!(
                "invalid price for {symbol}: {current_price_cents} cents"
            )));
        }

        // quantity = size_usd / current_price  (both in cents, so the units cancel)
        let quantity = Decimal::from(size_usd_cents) / Decimal::from(current_price_cents);

        info!(
            symbol           = symbol,
            size_usd_cents,
            current_price_cents,
            %quantity,
            "opening short (market sell)"
        );

        self.binance
            .market_sell(symbol, quantity)
            .await
            .map_err(|e| TokenUnlockError::TradeError(format!("market sell failed for {symbol}: {e}")))
    }

    /// Close a short position by placing a market buy.
    pub async fn close_short(
        &self,
        symbol: &str,
        quantity: Decimal,
    ) -> Result<OrderResult, TokenUnlockError> {
        info!(symbol, %quantity, "closing short (market buy)");

        self.binance
            .market_buy(symbol, quantity)
            .await
            .map_err(|e| TokenUnlockError::TradeError(format!("market buy failed for {symbol}: {e}")))
    }

    // -----------------------------------------------------------------------
    // Sizing
    // -----------------------------------------------------------------------

    /// Calculate position size as 10% of the unlock value (integer arithmetic).
    pub fn position_size_cents(&self, unlock_usd_cents: i64) -> i64 {
        unlock_usd_cents / 10
    }
}
