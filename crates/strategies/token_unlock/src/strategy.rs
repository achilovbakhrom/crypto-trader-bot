use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use rust_decimal::Decimal;
use tracing::{debug, error, info, warn};

use trader_core::types::Symbol;
use feeds::price_store::PriceStore;
use monitor::metrics::Metrics;
use storage::db::Database;
use storage::models::NewTokenUnlock;
use storage::queries;

use crate::error::TokenUnlockError;
use crate::scanner::UnlockScanner;
use crate::trader::UnlockTrader;

// ---------------------------------------------------------------------------
// Open-position tracking
// ---------------------------------------------------------------------------

/// An open short position being held by the strategy.
#[derive(Debug, Clone)]
struct OpenPosition {
    symbol: String,
    quantity: Decimal,
    /// Unix timestamp of the scheduled unlock.
    unlock_timestamp: i64,
}

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/// Orchestrates the token-unlock trading strategy.
///
/// Every `scan_interval_secs` the strategy:
/// 1. Scans all vesting contracts for upcoming unlock events.
/// 2. Evaluates each event with the trader's `should_trade` filter.
/// 3. Opens short positions for qualifying events and records them to the DB.
/// 4. Closes any open positions where the unlock date has passed.
pub struct TokenUnlockStrategy {
    scanner: UnlockScanner,
    trader: Arc<UnlockTrader>,
    db: Arc<Database>,
    metrics: Arc<Metrics>,
    price_store: Arc<PriceStore>,
    scan_interval_secs: u64,
}

impl TokenUnlockStrategy {
    /// Create a new strategy instance.
    pub fn new(
        scanner: UnlockScanner,
        trader: Arc<UnlockTrader>,
        db: Arc<Database>,
        metrics: Arc<Metrics>,
        price_store: Arc<PriceStore>,
        scan_interval_secs: u64,
    ) -> Self {
        Self {
            scanner,
            trader,
            db,
            metrics,
            price_store,
            scan_interval_secs,
        }
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------

    /// Run the strategy indefinitely.
    ///
    /// The loop never returns `Err` for transient failures; individual errors
    /// are logged and the loop continues so the bot stays alive.
    pub async fn run(&self) -> anyhow::Result<()> {
        let interval = tokio::time::Duration::from_secs(self.scan_interval_secs);

        info!(
            scan_interval_secs = self.scan_interval_secs,
            "TokenUnlockStrategy started"
        );

        // Track open positions keyed by symbol.
        let mut open_positions: HashMap<String, OpenPosition> = HashMap::new();

        loop {
            let now = Utc::now();

            // ------------------------------------------------------------------
            // 1. Close any positions whose unlock time has passed (days_before_exit
            //    logic is handled by should_trade on the entry side; here we close
            //    positions that are at or past the actual unlock timestamp).
            // ------------------------------------------------------------------
            self.close_expired_positions(&mut open_positions, now.timestamp())
                .await;

            // ------------------------------------------------------------------
            // 2. Scan vesting contracts for upcoming unlocks.
            // ------------------------------------------------------------------
            let events = match self.scanner.scan().await {
                Ok(events) => {
                    info!(count = events.len(), "scanned unlock events");
                    events
                }
                Err(e) => {
                    error!(error = %e, "failed to scan unlock events; retrying next interval");
                    tokio::time::sleep(interval).await;
                    continue;
                }
            };

            // Update unlocks_tracked metric.
            self.metrics
                .unlocks_tracked
                .store(events.len() as u64, std::sync::atomic::Ordering::Relaxed);

            // ------------------------------------------------------------------
            // 3. Evaluate each event and open positions as appropriate.
            // ------------------------------------------------------------------
            for event in &events {
                // Skip symbols we already hold a short in.
                if open_positions.contains_key(&event.symbol.0) {
                    debug!(symbol = %event.symbol, "already holding position; skipping");
                    continue;
                }

                if !self.trader.should_trade(event, now) {
                    continue;
                }

                // Fetch current price from the price store.
                let symbol = Symbol::new(format!("{}USDT", event.symbol));
                let price_cents = match self.price_store.ask(&symbol).await {
                    Some(price) => {
                        // Convert Decimal price (e.g. 1234.56) to integer cents (123456).
                        let cents = price.0 * Decimal::from(100);
                        match cents.try_into() {
                            Ok(c) => c,
                            Err(e) => {
                                warn!(
                                    symbol = %event.symbol,
                                    error  = %e,
                                    "failed to convert price to cents; skipping"
                                );
                                continue;
                            }
                        }
                    }
                    None => {
                        warn!(
                            symbol = %event.symbol,
                            "no price available in price store; skipping"
                        );
                        continue;
                    }
                };

                let size_cents = self.trader.position_size_cents(event.estimated_usd_cents);

                if size_cents <= 0 {
                    warn!(
                        symbol     = %event.symbol,
                        usd_cents  = event.estimated_usd_cents,
                        size_cents,
                        "computed zero position size; skipping"
                    );
                    continue;
                }

                // ------------------------------------------------------------------
                // 4. Open the short position.
                // ------------------------------------------------------------------
                let trading_symbol = format!("{}USDT", event.symbol);
                match self
                    .trader
                    .open_short(&trading_symbol, size_cents, price_cents)
                    .await
                {
                    Ok(order_result) => {
                        info!(
                            symbol    = %event.symbol,
                            order_id  = order_result.order_id,
                            qty       = %order_result.executed_qty,
                            "short position opened"
                        );

                        open_positions.insert(
                            event.symbol.0.clone(),
                            OpenPosition {
                                symbol: trading_symbol.clone(),
                                quantity: order_result.executed_qty,
                                unlock_timestamp: event.unlock_timestamp,
                            },
                        );

                        // ------------------------------------------------------------------
                        // 5. Persist the unlock event to the database.
                        // ------------------------------------------------------------------
                        let unlock_at = chrono::DateTime::from_timestamp(event.unlock_timestamp, 0)
                            .unwrap_or(now);

                        // Wei is u128; Decimal has no From<u128>, so we go through string.
                        let amount_decimal = Decimal::from_str_exact(
                            &event.unlock_amount_wei.0.to_string(),
                        )
                        .unwrap_or(Decimal::ZERO);

                        let new_unlock = NewTokenUnlock {
                            token_address: event.token_address.clone(),
                            symbol: event.symbol.0.clone(),
                            unlock_at,
                            amount_wei: amount_decimal,
                            usd_value_cents: event.estimated_usd_cents,
                            beneficiary_type: format!("{:?}", event.beneficiary_type),
                            trade_id: None,
                            created_at: now,
                        };

                        if let Err(e) =
                            queries::insert_token_unlock(self.db.pool(), new_unlock).await
                        {
                            warn!(
                                symbol = %event.symbol,
                                error  = %e,
                                "failed to persist token unlock to database"
                            );
                        }

                        // Log a generic system event.
                        let payload = serde_json::json!({
                            "symbol": event.symbol.0,
                            "token_address": event.token_address,
                            "unlock_timestamp": event.unlock_timestamp,
                            "usd_cents": event.estimated_usd_cents,
                            "size_cents": size_cents,
                            "order_id": order_result.order_id,
                        });

                        if let Err(e) =
                            queries::log_event(self.db.pool(), "token_unlock.short_opened", payload)
                                .await
                        {
                            warn!(error = %e, "failed to log system event");
                        }
                    }
                    Err(e) => {
                        error!(
                            symbol = %event.symbol,
                            error  = %e,
                            "failed to open short position"
                        );
                    }
                }
            }

            // ------------------------------------------------------------------
            // 6. Sleep until next scan cycle.
            // ------------------------------------------------------------------
            debug!(interval_secs = self.scan_interval_secs, "sleeping until next scan");
            tokio::time::sleep(interval).await;
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Close all open positions whose unlock timestamp is in the past.
    async fn close_expired_positions(
        &self,
        open_positions: &mut HashMap<String, OpenPosition>,
        now_ts: i64,
    ) {
        let expired: Vec<String> = open_positions
            .iter()
            .filter(|(_, pos)| pos.unlock_timestamp <= now_ts)
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired {
            let pos = match open_positions.remove(&key) {
                Some(p) => p,
                None => continue,
            };

            info!(
                symbol           = %pos.symbol,
                unlock_timestamp = pos.unlock_timestamp,
                "closing short position (unlock passed)"
            );

            match self.trader.close_short(&pos.symbol, pos.quantity).await {
                Ok(order_result) => {
                    info!(
                        symbol   = %pos.symbol,
                        order_id = order_result.order_id,
                        qty      = %order_result.executed_qty,
                        "short position closed"
                    );

                    let payload = serde_json::json!({
                        "symbol": pos.symbol,
                        "quantity": pos.quantity.to_string(),
                        "order_id": order_result.order_id,
                    });

                    if let Err(e) = queries::log_event(
                        self.db.pool(),
                        "token_unlock.short_closed",
                        payload,
                    )
                    .await
                    {
                        warn!(error = %e, "failed to log short-closed event");
                    }
                }
                Err(e) => {
                    error!(
                        symbol = %pos.symbol,
                        error  = %e,
                        "failed to close short position; re-inserting for retry"
                    );
                    // Re-insert so we attempt to close it on the next iteration.
                    open_positions.insert(key, pos);
                }
            }
        }
    }
}
