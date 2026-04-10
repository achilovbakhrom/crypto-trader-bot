use std::sync::Arc;

use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use storage::models::NewLiquidation;
use trader_core::types::{BorrowerPosition, LiquidationOpportunity};

use crate::detector::LiquidationDetector;
use crate::executor::LiquidationExecutor;

/// Top-level strategy that wires monitor -> detector -> executor -> storage.
pub struct LiquidationStrategy {
    monitor: Arc<chain::aave::AaveMonitor>,
    detector: LiquidationDetector,
    executor: Arc<LiquidationExecutor>,
    db: Arc<storage::db::Database>,
    metrics: Arc<monitor::metrics::Metrics>,
    price_store: Arc<feeds::price_store::PriceStore>,
    min_profit_cents: i64,
}

impl LiquidationStrategy {
    /// Construct a new `LiquidationStrategy`.
    pub fn new(
        monitor: Arc<chain::aave::AaveMonitor>,
        detector: LiquidationDetector,
        executor: Arc<LiquidationExecutor>,
        db: Arc<storage::db::Database>,
        metrics: Arc<monitor::metrics::Metrics>,
        price_store: Arc<feeds::price_store::PriceStore>,
        min_profit_cents: i64,
    ) -> Self {
        Self {
            monitor,
            detector,
            executor,
            db,
            metrics,
            price_store,
            min_profit_cents,
        }
    }

    /// Return a reference to the underlying [`chain::aave::AaveMonitor`].
    ///
    /// Callers can use this to spawn `monitor.run(opportunity_tx)` as a
    /// background task before calling `strategy.run(opportunity_rx, provider)`.
    pub fn monitor(&self) -> &chain::aave::AaveMonitor {
        &self.monitor
    }

    /// Main event loop.
    ///
    /// Receives [`LiquidationOpportunity`] values from `opportunity_rx`,
    /// evaluates each one via the detector (using a live price from the price
    /// store), executes profitable ones, and records every attempt in the
    /// database while updating metrics.
    pub async fn run(
        &self,
        mut opportunity_rx: mpsc::Receiver<LiquidationOpportunity>,
        provider: Arc<impl alloy::providers::Provider + 'static>,
    ) -> anyhow::Result<()> {
        info!("LiquidationStrategy run loop started");

        while let Some(opportunity) = opportunity_rx.recv().await {
            info!(
                borrower      = %opportunity.borrower,
                health_factor = opportunity.health_factor.0,
                collateral    = %opportunity.collateral_asset,
                debt_asset    = %opportunity.debt_asset,
                "received liquidation opportunity"
            );

            // ------------------------------------------------------------------
            // Step 1: Get current collateral price from the price store.
            // ------------------------------------------------------------------
            let symbol = &opportunity.collateral_symbol;
            let price_cents = match self.price_store.bid(symbol).await {
                Some(p) => {
                    // Convert the Decimal bid price (in USDT) to integer cents.
                    let cents_decimal = p.0 * Decimal::new(100, 0);
                    match cents_decimal.to_i64() {
                        Some(c) => c,
                        None => {
                            warn!(
                                symbol = %symbol,
                                "collateral price out of i64 range; skipping"
                            );
                            continue;
                        }
                    }
                }
                None => {
                    warn!(
                        symbol = %symbol,
                        "no price available for collateral symbol; skipping opportunity"
                    );
                    continue;
                }
            };

            // ------------------------------------------------------------------
            // Step 2: Evaluate with the detector.
            // ------------------------------------------------------------------
            // The channel carries LiquidationOpportunity (not BorrowerPosition).
            // Reconstruct a minimal BorrowerPosition so the detector can apply
            // its size and profit checks.
            let position = opportunity_to_position(&opportunity);
            let evaluated = self
                .detector
                .evaluate(&position, price_cents, symbol.0.as_str());

            let evaluated_opportunity = match evaluated {
                Some(opp) => opp,
                None => {
                    info!(
                        borrower    = %opportunity.borrower,
                        price_cents,
                        min_profit  = self.min_profit_cents,
                        "opportunity does not meet profit threshold; skipping"
                    );
                    continue;
                }
            };

            // ------------------------------------------------------------------
            // Step 3: Log the attempt to the database.
            // ------------------------------------------------------------------
            self.metrics.record_liquidation_attempt();

            let new_liq = NewLiquidation {
                borrower: evaluated_opportunity.borrower.clone(),
                protocol: format!("{}", evaluated_opportunity.protocol),
                collateral_asset: evaluated_opportunity.collateral_asset.clone(),
                debt_asset: evaluated_opportunity.debt_asset.clone(),
                debt_amount_wei: Decimal::from(
                    evaluated_opportunity
                        .debt_amount_wei
                        .0
                        .min(i64::MAX as u128) as i64,
                ),
                collateral_received_wei: Decimal::ZERO,
                profit_cents: evaluated_opportunity.estimated_profit_cents,
                status: "pending".to_string(),
                tx_hash: None,
                executed_at: Utc::now(),
                error: None,
            };

            let record_id =
                match storage::queries::insert_liquidation(self.db.pool(), new_liq).await {
                    Ok(id) => {
                        info!(%id, "liquidation attempt logged to DB");
                        id
                    }
                    Err(e) => {
                        error!(error = %e, "failed to insert liquidation attempt into DB");
                        // Non-fatal: continue with execution.
                        uuid::Uuid::nil()
                    }
                };

            // Log the attempt as a system event too.
            let attempt_payload = json!({
                "borrower":               &evaluated_opportunity.borrower,
                "protocol":               format!("{}", evaluated_opportunity.protocol),
                "estimated_profit_cents": evaluated_opportunity.estimated_profit_cents,
                "debt_amount_wei":        evaluated_opportunity.debt_amount_wei.0.to_string(),
            });
            if let Err(e) =
                storage::queries::log_event(self.db.pool(), "liquidation.attempt", attempt_payload)
                    .await
            {
                error!(error = %e, "failed to log liquidation.attempt event");
            }

            // ------------------------------------------------------------------
            // Step 4: Execute the liquidation.
            // ------------------------------------------------------------------
            match self
                .executor
                .execute(&evaluated_opportunity, &*provider)
                .await
            {
                Ok(result) => {
                    // --------------------------------------------------------
                    // Step 5 (success): log result to DB, update metrics.
                    // --------------------------------------------------------
                    info!(
                        tx_hash       = %result.tx_hash,
                        actual_profit = result.actual_profit_cents,
                        order_id      = result.sell_order.order_id,
                        "liquidation executed successfully"
                    );

                    self.metrics
                        .record_liquidation_success(result.actual_profit_cents);

                    if !record_id.is_nil() {
                        if let Err(e) = storage::queries::update_liquidation_status(
                            self.db.pool(),
                            record_id,
                            "success",
                            Some(result.tx_hash.as_str()),
                            None,
                        )
                        .await
                        {
                            error!(error = %e, %record_id, "failed to update liquidation status to success");
                        }
                    }

                    let success_payload = json!({
                        "borrower":           &evaluated_opportunity.borrower,
                        "tx_hash":            &result.tx_hash,
                        "actual_profit_cents": result.actual_profit_cents,
                        "order_id":           result.sell_order.order_id,
                        "executed_qty":       result.sell_order.executed_qty.to_string(),
                    });
                    if let Err(e) = storage::queries::log_event(
                        self.db.pool(),
                        "liquidation.success",
                        success_payload,
                    )
                    .await
                    {
                        error!(error = %e, "failed to log liquidation.success event");
                    }
                }

                Err(e) => {
                    // --------------------------------------------------------
                    // Step 6 (error): log error to DB, update failure metric.
                    // --------------------------------------------------------
                    error!(
                        error    = %e,
                        borrower = %evaluated_opportunity.borrower,
                        "liquidation execution failed"
                    );

                    self.metrics.record_liquidation_failure();

                    if !record_id.is_nil() {
                        if let Err(db_err) = storage::queries::update_liquidation_status(
                            self.db.pool(),
                            record_id,
                            "failed",
                            None,
                            Some(e.to_string().as_str()),
                        )
                        .await
                        {
                            error!(error = %db_err, %record_id, "failed to update liquidation status to failed");
                        }
                    }

                    let fail_payload = json!({
                        "borrower": &evaluated_opportunity.borrower,
                        "error":    e.to_string(),
                    });
                    if let Err(db_err) = storage::queries::log_event(
                        self.db.pool(),
                        "liquidation.failed",
                        fail_payload,
                    )
                    .await
                    {
                        error!(error = %db_err, "failed to log liquidation.failed event");
                    }
                }
            }
        }

        info!("LiquidationStrategy run loop terminated: opportunity channel closed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build a minimal [`BorrowerPosition`] from a [`LiquidationOpportunity`].
///
/// The monitor emits opportunities already confirmed to be liquidatable
/// (health_factor < 1e18). We reconstruct a position so `max_liquidatable_debt()`
/// returns the same `debt_amount_wei` that was in the opportunity:
///   max_liquidatable_debt = total_debt_wei / 2
///   => total_debt_wei = debt_amount_wei * 2
fn opportunity_to_position(opp: &LiquidationOpportunity) -> BorrowerPosition {
    use trader_core::types::Wei;

    let total_debt_wei = Wei(opp.debt_amount_wei.0.saturating_mul(2));

    BorrowerPosition {
        address: opp.borrower.clone(),
        protocol: opp.protocol,
        health_factor: opp.health_factor,
        collateral_asset: opp.collateral_asset.clone(),
        debt_asset: opp.debt_asset.clone(),
        // Use debt as a proxy for collateral (we don't have the real value here).
        total_collateral_wei: opp.debt_amount_wei,
        total_debt_wei,
        liquidation_threshold_bps: 8000, // Aave V3 default (80%)
    }
}
