use trader_core::types::{BorrowerPosition, LiquidationOpportunity, Wei};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Evaluates whether a borrower position meets the profit threshold for liquidation.
pub struct LiquidationDetector {
    min_profit_cents: i64,
    max_position_usd_cents: i64,
    /// Liquidation bonus in bps (500 = 5%)
    bonus_bps: u32,
}

impl LiquidationDetector {
    /// Create a new detector.
    ///
    /// * `min_profit_cents`      – minimum net profit in USDT cents required to proceed.
    /// * `max_position_usd_cents`– positions larger than this (in cents) are skipped to
    ///                             limit capital exposure.
    pub fn new(min_profit_cents: i64, max_position_usd_cents: i64) -> Self {
        Self {
            min_profit_cents,
            max_position_usd_cents,
            bonus_bps: 500,
        }
    }

    /// Given a position and the current collateral price in USD cents, returns
    /// `Some(opportunity)` if the liquidation is estimated to be profitable,
    /// `None` otherwise.
    ///
    /// Checks performed:
    /// 1. Position must be liquidatable (health factor < 1e18).
    /// 2. Estimated position value must not exceed `max_position_usd_cents`.
    /// 3. Estimated profit must exceed `min_profit_cents`.
    pub fn evaluate(
        &self,
        position: &BorrowerPosition,
        collateral_price_cents: i64,
        cex_symbol: &str,
    ) -> Option<LiquidationOpportunity> {
        if !position.is_liquidatable() {
            return None;
        }

        let debt_to_cover = position.max_liquidatable_debt();

        // Estimate the USD value of the position in cents.
        // total_collateral_wei is stored in Aave's base unit (8-decimal USD),
        // so 1e8 = $1.  Multiply by collateral_price_cents / 1e8 to get cents.
        let collateral_value_cents = self.collateral_value_cents(
            position.total_collateral_wei,
            collateral_price_cents,
        );

        if collateral_value_cents > self.max_position_usd_cents {
            return None;
        }

        let gross_profit = self.calculate_profit_cents(debt_to_cover, collateral_price_cents);

        if gross_profit < self.min_profit_cents {
            return None;
        }

        Some(LiquidationOpportunity {
            borrower: position.address.clone(),
            protocol: position.protocol,
            collateral_asset: position.collateral_asset.clone(),
            collateral_symbol: trader_core::types::Symbol::new(cex_symbol),
            debt_asset: position.debt_asset.clone(),
            debt_amount_wei: debt_to_cover,
            health_factor: position.health_factor,
            bonus_bps: self.bonus_bps,
            estimated_profit_cents: gross_profit,
        })
    }

    /// Calculate gross profit in cents from liquidating `debt_amount_wei` of debt.
    ///
    /// Formula:
    ///   profit = (debt_amount_wei / 1e18) * (bonus_bps / 10_000) * collateral_price_cents
    ///
    /// `debt_amount_wei` is denominated in the debt asset (18 decimals).
    /// The result is in USDT cents.
    pub fn calculate_profit_cents(
        &self,
        debt_amount_wei: Wei,
        collateral_price_cents: i64,
    ) -> i64 {
        // Convert debt from wei (u128, 18 decimals) to a Decimal.
        let debt_decimal = Decimal::new(
            debt_amount_wei.0.min(i64::MAX as u128) as i64,
            18,
        );

        let bonus = Decimal::new(self.bonus_bps as i64, 4); // bonus_bps / 10_000
        let price = Decimal::new(collateral_price_cents, 0);

        let profit = debt_decimal * bonus * price;
        profit.to_i64().unwrap_or(0)
    }

    /// Estimate gas cost in USD cents.
    ///
    /// Formula: 200_000 gas × gas_price_gwei × 1e-9 ETH/gas × eth_price_cents
    pub fn estimate_gas_cents(gas_price_gwei: u64, eth_price_cents: i64) -> i64 {
        // gas_cost_eth = 200_000 * gas_price_gwei * 1e-9
        // gas_cost_cents = gas_cost_eth * eth_price_cents
        let gas_units = Decimal::new(200_000, 0);
        let gwei = Decimal::new(gas_price_gwei as i64, 0);
        // 1 gwei = 1e-9 ETH
        let eth_per_gwei = Decimal::new(1, 9);
        let eth_price = Decimal::new(eth_price_cents, 0);

        let cost = gas_units * gwei * eth_per_gwei * eth_price;
        cost.to_i64().unwrap_or(0)
    }

    /// Convert Aave's `total_collateral_wei` to USDT cents.
    ///
    /// Aave stores collateral in its base currency (8-decimal USD):
    ///   1e8 base units = $1 => value_cents = (raw / 1e8) * 100
    ///
    /// The market `_collateral_price_cents` is not used because Aave's base
    /// currency is already denominated in USD.
    fn collateral_value_cents(&self, total_collateral_wei: Wei, _collateral_price_cents: i64) -> i64 {
        // total_collateral_wei with scale 8 gives USD value.
        // Multiply by 100 to convert USD to cents.
        let raw = Decimal::new(
            total_collateral_wei.0.min(i64::MAX as u128) as i64,
            8,
        );
        let usd_cents = raw * Decimal::new(100, 0);
        usd_cents.to_i64().unwrap_or(i64::MAX)
    }
}
