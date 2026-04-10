use std::collections::HashMap;
use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::ws::WsConnect;
use futures::StreamExt;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};
use url::Url;

use trader_core::types::{BorrowerPosition, LendingProtocol, LiquidationOpportunity, Symbol, Wei};

use crate::aave::types::IAavePool;
use crate::error::ChainError;

/// Liquidation bonus in basis points for Aave V3 (5%).
/// The actual value is asset-specific; this constant is used as a conservative estimate.
const AAVE_LIQUIDATION_BONUS_BPS: u32 = 500;

/// A record of a known borrower, used to seed `getUserAccountData` calls.
#[derive(Debug, Clone)]
struct BorrowerRecord {
    address: String,
    collateral_asset: String,
    debt_asset: String,
}

/// Monitors Aave V3 borrower positions on an EVM-compatible chain.
///
/// Uses an HTTP provider for periodic polling and a WebSocket provider
/// (derived from the RPC URL by replacing the scheme) for streaming `Borrow`
/// events to discover new borrowers automatically.
pub struct AaveMonitor {
    /// HTTP RPC URL. The WebSocket URL is derived by replacing the scheme.
    rpc_url: String,
    /// Aave V3 Pool contract address.
    pool_address: Address,
    /// Known borrowers to monitor. Key = lowercased hex address.
    borrowers: Arc<RwLock<HashMap<String, BorrowerRecord>>>,
    /// Milliseconds between full polling sweeps.
    poll_interval_ms: u64,
}

impl AaveMonitor {
    /// Construct a new monitor.
    ///
    /// * `rpc_url`          – HTTP JSON-RPC endpoint (e.g. Base mainnet Alchemy URL).
    /// * `pool_address`     – Aave V3 Pool contract address (0x-prefixed hex string).
    /// * `poll_interval_ms` – How often (ms) to sweep all known borrowers.
    pub async fn new(
        rpc_url: &str,
        pool_address: &str,
        poll_interval_ms: u64,
    ) -> Result<Self, ChainError> {
        let pool_address: Address = pool_address.parse().map_err(|e| {
            ChainError::ParseError(format!("Invalid pool address '{}': {}", pool_address, e))
        })?;

        // Validate the URL up front so we fail fast rather than at poll time.
        let _: Url = rpc_url
            .parse()
            .map_err(|e| ChainError::ParseError(format!("Invalid RPC URL '{}': {}", rpc_url, e)))?;

        Ok(Self {
            rpc_url: rpc_url.to_string(),
            pool_address,
            borrowers: Arc::new(RwLock::new(HashMap::new())),
            poll_interval_ms,
        })
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Build a fresh HTTP provider. This is cheap – alloy's HTTP provider is
    /// a thin wrapper around a `reqwest::Client`.
    fn http_provider(&self) -> Result<impl Provider + Clone, ChainError> {
        let url: Url = self
            .rpc_url
            .parse()
            .map_err(|e| ChainError::ParseError(format!("Invalid RPC URL: {}", e)))?;
        Ok(ProviderBuilder::new().connect_http(url))
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Add a borrower address to the watch list.
    ///
    /// If the address is already tracked, its asset hints are updated.
    pub async fn add_borrower(
        &self,
        address: String,
        collateral_asset: String,
        debt_asset: String,
    ) {
        let key = address.to_lowercase();
        let mut guard = self.borrowers.write().await;
        guard.insert(
            key,
            BorrowerRecord {
                address,
                collateral_asset,
                debt_asset,
            },
        );
    }

    /// Return a snapshot of all currently tracked positions by calling
    /// `getUserAccountData` on-chain for each known borrower.
    pub async fn positions(&self) -> Vec<BorrowerPosition> {
        let records: Vec<BorrowerRecord> = {
            let guard = self.borrowers.read().await;
            guard.values().cloned().collect()
        };

        let mut positions = Vec::with_capacity(records.len());
        for record in records {
            match self.check_position(&record.address).await {
                Ok(pos) => positions.push(pos),
                Err(e) => {
                    warn!(
                        address = %record.address,
                        error   = %e,
                        "Failed to fetch position snapshot"
                    );
                }
            }
        }
        positions
    }

    /// Check a single borrower's on-chain position via `getUserAccountData`.
    async fn check_position(&self, address: &str) -> Result<BorrowerPosition, ChainError> {
        let provider = self.http_provider()?;
        let pool = IAavePool::new(self.pool_address, provider);

        let user: Address = address.parse().map_err(|e| {
            ChainError::ParseError(format!("Invalid borrower address '{}': {}", address, e))
        })?;

        let data =
            pool.getUserAccountData(user).call().await.map_err(|e| {
                ChainError::ContractError(format!("getUserAccountData failed: {}", e))
            })?;

        let (collateral_asset, debt_asset) = {
            let guard = self.borrowers.read().await;
            guard
                .get(&address.to_lowercase())
                .map(|r| (r.collateral_asset.clone(), r.debt_asset.clone()))
                .unwrap_or_default()
        };

        // Aave V3 returns healthFactor in 18-decimal fixed-point (1e18 = 1.0).
        // The collateral and debt values are in the oracle base currency (USD, 8 dec).
        // We store raw u128 values and let the execution layer normalise as needed.
        let health_factor: u128 = u128::try_from(data.healthFactor).unwrap_or(u128::MAX);
        let total_collateral: u128 = u128::try_from(data.totalCollateralBase).unwrap_or(0);
        let total_debt: u128 = u128::try_from(data.totalDebtBase).unwrap_or(0);
        // currentLiquidationThreshold is returned in basis points (e.g. 8000 = 80%).
        let liq_threshold: u128 = u128::try_from(data.currentLiquidationThreshold).unwrap_or(0);
        let liquidation_threshold_bps = liq_threshold as u32;

        Ok(BorrowerPosition {
            address: address.to_string(),
            protocol: LendingProtocol::AaveV3,
            health_factor: Wei(health_factor),
            collateral_asset,
            debt_asset,
            total_collateral_wei: Wei(total_collateral),
            total_debt_wei: Wei(total_debt),
            liquidation_threshold_bps,
        })
    }

    /// Subscribe to on-chain `Borrow` events and register newly seen borrowers.
    ///
    /// The WebSocket URL is derived from `rpc_url` by replacing the scheme
    /// (`https://` → `wss://`, `http://` → `ws://`). This method runs
    /// indefinitely and must be spawned as a background task.
    pub async fn watch_new_borrowers(&self) -> Result<(), ChainError> {
        let ws_url = self
            .rpc_url
            .replace("https://", "wss://")
            .replace("http://", "ws://");

        let ws_provider = ProviderBuilder::new()
            .connect_ws(WsConnect::new(&ws_url))
            .await
            .map_err(|e| {
                ChainError::ProviderError(format!("WebSocket connection failed: {}", e))
            })?;

        let pool = IAavePool::new(self.pool_address, ws_provider);

        let mut stream = pool
            .Borrow_filter()
            .subscribe()
            .await
            .map_err(|e| {
                ChainError::RpcError(format!("Failed to subscribe to Borrow events: {}", e))
            })?
            .into_stream();

        info!(pool = %self.pool_address, "Subscribed to Aave V3 Borrow events");

        while let Some(log_result) = stream.next().await {
            match log_result {
                Ok((event, _log)) => {
                    // `onBehalfOf` is the account whose debt is recorded (the borrower).
                    let borrower_addr = event.onBehalfOf.to_string();
                    // The reserve is the asset being borrowed → this is the debt asset.
                    let reserve_addr = event.reserve.to_string();

                    debug!(
                        borrower = %borrower_addr,
                        reserve  = %reserve_addr,
                        amount   = %event.amount,
                        "New Borrow event detected"
                    );

                    // Collateral asset is not available from the Borrow event alone.
                    // getUserAccountData provides the accounting; leave it empty.
                    self.add_borrower(
                        borrower_addr,
                        String::new(), // collateral unknown from Borrow event
                        reserve_addr,  // borrowed reserve = debt asset
                    )
                    .await;
                }
                Err(e) => {
                    error!(error = %e, "Error receiving Borrow event");
                }
            }
        }

        warn!("Borrow event stream ended unexpectedly");
        Ok(())
    }

    /// Main monitoring loop.
    ///
    /// 1. Spawns a background task to watch for `Borrow` events (populates
    ///    the borrower registry automatically).
    /// 2. Periodically polls all known borrowers' health factors via HTTP.
    /// 3. Sends a [`LiquidationOpportunity`] to `opportunity_tx` for any
    ///    position whose health factor has fallen below 1e18 (liquidatable).
    pub async fn run(
        &self,
        opportunity_tx: mpsc::Sender<LiquidationOpportunity>,
    ) -> Result<(), ChainError> {
        // Spawn the event watcher. We reconstruct a lightweight inner monitor
        // that shares the same `borrowers` map via Arc so newly seen accounts
        // are immediately visible to the polling loop below.
        {
            let rpc_url = self.rpc_url.clone();
            let pool_address = self.pool_address;
            let borrowers = Arc::clone(&self.borrowers);
            let poll_interval_ms = self.poll_interval_ms;

            tokio::spawn(async move {
                let watcher = AaveMonitor {
                    rpc_url,
                    pool_address,
                    borrowers,
                    poll_interval_ms,
                };
                if let Err(e) = watcher.watch_new_borrowers().await {
                    error!(error = %e, "Borrow event watcher terminated with error");
                }
            });
        }

        info!(
            pool        = %self.pool_address,
            interval_ms = self.poll_interval_ms,
            "AaveMonitor polling loop started"
        );

        let sleep_duration = tokio::time::Duration::from_millis(self.poll_interval_ms);

        loop {
            let records: Vec<BorrowerRecord> = {
                let guard = self.borrowers.read().await;
                guard.values().cloned().collect()
            };

            debug!(borrower_count = records.len(), "Starting polling sweep");

            for record in records {
                match self.check_position(&record.address).await {
                    Ok(position) => {
                        if position.is_liquidatable() {
                            let opportunity = build_opportunity(&position);
                            info!(
                                borrower      = %position.address,
                                health_factor = position.health_factor.0,
                                debt_wei      = position.total_debt_wei.0,
                                "Liquidation opportunity detected"
                            );
                            if let Err(send_err) = opportunity_tx.send(opportunity).await {
                                error!(
                                    error = %send_err,
                                    "Failed to send liquidation opportunity; channel closed"
                                );
                                return Err(ChainError::ProviderError(
                                    "Opportunity channel closed".into(),
                                ));
                            }
                        } else {
                            debug!(
                                borrower      = %position.address,
                                health_factor = position.health_factor.0,
                                "Position is healthy"
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            borrower = %record.address,
                            error    = %e,
                            "Failed to check borrower position"
                        );
                    }
                }
            }

            tokio::time::sleep(sleep_duration).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Construct a [`LiquidationOpportunity`] from a liquidatable [`BorrowerPosition`].
fn build_opportunity(position: &BorrowerPosition) -> LiquidationOpportunity {
    // Cap the covered debt at 50% of total debt (Aave's default close factor).
    let debt_amount_wei = position.max_liquidatable_debt();

    // Best-effort symbol lookup for the collateral asset.
    let collateral_symbol = symbol_for_address(&position.collateral_asset);

    // Price is unknown at this layer; the execution layer fills in the profit.
    LiquidationOpportunity {
        borrower: position.address.clone(),
        protocol: position.protocol,
        collateral_asset: position.collateral_asset.clone(),
        collateral_symbol,
        debt_asset: position.debt_asset.clone(),
        debt_amount_wei,
        health_factor: position.health_factor,
        bonus_bps: AAVE_LIQUIDATION_BONUS_BPS,
        estimated_profit_cents: 0,
    }
}

/// Return a best-effort [`Symbol`] for a well-known Base mainnet token address.
fn symbol_for_address(addr: &str) -> Symbol {
    use crate::aave::types::{CBBTC_BASE, USDC_BASE, WETH_BASE};

    let lower = addr.to_lowercase();
    let sym = if lower == WETH_BASE.to_lowercase() {
        "WETH"
    } else if lower == USDC_BASE.to_lowercase() {
        "USDC"
    } else if lower == CBBTC_BASE.to_lowercase() {
        "cbBTC"
    } else {
        "UNKNOWN"
    };
    Symbol::new(sym)
}
