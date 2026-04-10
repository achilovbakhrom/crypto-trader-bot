use std::sync::Arc;

use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::sol;
use alloy::sol_types::SolEvent;
use tracing::{debug, info, warn};

use trader_core::types::{BeneficiaryType, Symbol, TokenUnlockEvent, Wei};

use crate::error::TokenUnlockError;

// ---------------------------------------------------------------------------
// Standard vesting ABI events
// ---------------------------------------------------------------------------

sol! {
    event TokensReleased(address indexed token, uint256 amount);
    event VestingScheduleCreated(
        address indexed beneficiary,
        uint256 start,
        uint256 cliff,
        uint256 duration,
        uint256 slicePeriodSeconds,
        bool revocable,
        uint256 amount
    );
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Known vesting contract to monitor.
#[derive(Debug, Clone)]
pub struct VestingContract {
    pub address: String,
    pub token_address: String,
    pub token_symbol: Symbol,
    pub beneficiary_type: BeneficiaryType,
    pub chain_id: u64,
}

/// Scans registered vesting contracts for upcoming unlock events.
pub struct UnlockScanner {
    contracts: Vec<VestingContract>,
    provider: Arc<dyn Provider + Send + Sync>,
}

impl UnlockScanner {
    /// Create a new scanner with the given contracts and provider.
    pub fn new(
        contracts: Vec<VestingContract>,
        provider: Arc<dyn Provider + Send + Sync>,
    ) -> Self {
        Self {
            contracts,
            provider,
        }
    }

    /// Scan all registered vesting contracts for upcoming unlock events.
    /// Returns events sorted by unlock timestamp ascending.
    pub async fn scan(&self) -> Result<Vec<TokenUnlockEvent>, TokenUnlockError> {
        let mut all_events: Vec<TokenUnlockEvent> = Vec::new();

        for contract in &self.contracts {
            match self.scan_contract(contract).await {
                Ok(mut events) => {
                    info!(
                        address = %contract.address,
                        symbol  = %contract.token_symbol,
                        count   = events.len(),
                        "scanned vesting contract"
                    );
                    all_events.append(&mut events);
                }
                Err(e) => {
                    warn!(
                        address = %contract.address,
                        error   = %e,
                        "failed to scan vesting contract; skipping"
                    );
                }
            }
        }

        // Sort ascending by unlock timestamp so callers process the soonest first.
        all_events.sort_by_key(|e| e.unlock_timestamp);

        debug!(total_events = all_events.len(), "scan complete");
        Ok(all_events)
    }

    /// Fetch the token release schedule from a single vesting contract.
    ///
    /// Queries for `TokensReleased` and `VestingScheduleCreated` events from
    /// block 0 to `latest`, then constructs [`TokenUnlockEvent`] records.
    async fn scan_contract(
        &self,
        contract: &VestingContract,
    ) -> Result<Vec<TokenUnlockEvent>, TokenUnlockError> {
        let contract_address: Address = contract
            .address
            .parse()
            .map_err(|e| TokenUnlockError::ScanError(format!("invalid contract address '{}': {}", contract.address, e)))?;

        // ------------------------------------------------------------------
        // Fetch the latest block number so we can bound the filter range.
        // ------------------------------------------------------------------
        let latest_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| TokenUnlockError::ScanError(format!("failed to get block number: {}", e)))?;

        debug!(
            contract = %contract.address,
            latest_block,
            "querying vesting events"
        );

        let mut events: Vec<TokenUnlockEvent> = Vec::new();

        // ------------------------------------------------------------------
        // Query TokensReleased events
        // ------------------------------------------------------------------
        let released_filter = alloy::rpc::types::Filter::new()
            .address(contract_address)
            .event_signature(TokensReleased::SIGNATURE_HASH)
            .from_block(0u64)
            .to_block(latest_block);

        let released_logs = self
            .provider
            .get_logs(&released_filter)
            .await
            .map_err(|e| TokenUnlockError::ScanError(format!("failed to fetch TokensReleased logs: {}", e)))?;

        debug!(
            contract = %contract.address,
            count    = released_logs.len(),
            "fetched TokensReleased logs"
        );

        for log in released_logs {
            let decoded = TokensReleased::decode_log(log.as_ref())
                .map_err(|e| TokenUnlockError::ScanError(format!("failed to decode TokensReleased log: {}", e)))?;

            // Use block timestamp from the log; fall back to 0 when unavailable.
            let unlock_timestamp = log
                .block_timestamp
                .map(|ts| ts as i64)
                .unwrap_or(0i64);

            let amount_u128: u128 = decoded.amount.try_into().unwrap_or(u128::MAX);

            events.push(TokenUnlockEvent {
                token_address: contract.token_address.clone(),
                symbol: contract.token_symbol.clone(),
                unlock_timestamp,
                unlock_amount_wei: Wei(amount_u128),
                // USD value is unknown without a price feed; callers fill it in.
                estimated_usd_cents: 0,
                beneficiary_type: contract.beneficiary_type,
            });
        }

        // ------------------------------------------------------------------
        // Query VestingScheduleCreated events
        // ------------------------------------------------------------------
        let schedule_filter = alloy::rpc::types::Filter::new()
            .address(contract_address)
            .event_signature(VestingScheduleCreated::SIGNATURE_HASH)
            .from_block(0u64)
            .to_block(latest_block);

        let schedule_logs = self
            .provider
            .get_logs(&schedule_filter)
            .await
            .map_err(|e| TokenUnlockError::ScanError(format!("failed to fetch VestingScheduleCreated logs: {}", e)))?;

        debug!(
            contract = %contract.address,
            count    = schedule_logs.len(),
            "fetched VestingScheduleCreated logs"
        );

        for log in schedule_logs {
            let decoded = VestingScheduleCreated::decode_log(log.as_ref())
                .map_err(|e| TokenUnlockError::ScanError(format!("failed to decode VestingScheduleCreated log: {}", e)))?;

            // The unlock happens at start + cliff + duration.
            let start_u64: u64 = decoded.start.try_into().unwrap_or(0u64);
            let cliff_u64: u64 = decoded.cliff.try_into().unwrap_or(0u64);
            let duration_u64: u64 = decoded.duration.try_into().unwrap_or(0u64);
            let unlock_timestamp = (start_u64 + cliff_u64 + duration_u64) as i64;

            let amount_u128: u128 = decoded.amount.try_into().unwrap_or(u128::MAX);

            events.push(TokenUnlockEvent {
                token_address: contract.token_address.clone(),
                symbol: contract.token_symbol.clone(),
                unlock_timestamp,
                unlock_amount_wei: Wei(amount_u128),
                estimated_usd_cents: 0,
                beneficiary_type: contract.beneficiary_type,
            });
        }

        Ok(events)
    }

    /// Add a vesting contract to monitor.
    pub fn add_contract(&mut self, contract: VestingContract) {
        info!(
            address = %contract.address,
            symbol  = %contract.token_symbol,
            "registered vesting contract"
        );
        self.contracts.push(contract);
    }
}
