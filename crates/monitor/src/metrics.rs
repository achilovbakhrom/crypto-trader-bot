use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// Shared, cheaply-cloneable set of atomic counters for the trading bot.
///
/// All counters use `Relaxed` ordering — they are advisory metrics and do not
/// participate in any synchronisation protocol that requires stronger ordering.
#[derive(Debug, Default, Clone)]
pub struct Metrics {
    pub liquidations_attempted: Arc<AtomicU64>,
    pub liquidations_succeeded: Arc<AtomicU64>,
    pub liquidations_failed: Arc<AtomicU64>,
    /// Cumulative profit in USD cents (can go negative).
    pub total_profit_cents: Arc<AtomicI64>,
    pub positions_monitored: Arc<AtomicU64>,
    pub price_updates_received: Arc<AtomicU64>,
    pub unlocks_tracked: Arc<AtomicU64>,
}

impl Metrics {
    /// Create a new zeroed `Metrics` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Increment the attempted-liquidation counter by 1.
    pub fn record_liquidation_attempt(&self) {
        self.liquidations_attempted.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the succeeded counter and add `profit_cents` to the running total.
    pub fn record_liquidation_success(&self, profit_cents: i64) {
        self.liquidations_succeeded.fetch_add(1, Ordering::Relaxed);
        self.total_profit_cents
            .fetch_add(profit_cents, Ordering::Relaxed);
    }

    /// Increment the failed-liquidation counter by 1.
    pub fn record_liquidation_failure(&self) {
        self.liquidations_failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Take an instantaneous snapshot of all counters.
    ///
    /// Because each counter is loaded independently there is a small window in
    /// which another thread may update a counter between loads.  This is
    /// acceptable for advisory metrics — callers that need a perfectly
    /// consistent view should quiesce all writers first.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            liquidations_attempted: self.liquidations_attempted.load(Ordering::Relaxed),
            liquidations_succeeded: self.liquidations_succeeded.load(Ordering::Relaxed),
            liquidations_failed: self.liquidations_failed.load(Ordering::Relaxed),
            total_profit_cents: self.total_profit_cents.load(Ordering::Relaxed),
            positions_monitored: self.positions_monitored.load(Ordering::Relaxed),
            price_updates_received: self.price_updates_received.load(Ordering::Relaxed),
            unlocks_tracked: self.unlocks_tracked.load(Ordering::Relaxed),
        }
    }
}

/// A point-in-time copy of all metric values, suitable for serialisation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub liquidations_attempted: u64,
    pub liquidations_succeeded: u64,
    pub liquidations_failed: u64,
    pub total_profit_cents: i64,
    pub positions_monitored: u64,
    pub price_updates_received: u64,
    pub unlocks_tracked: u64,
}
