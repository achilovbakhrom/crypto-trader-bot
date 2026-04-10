use alloy::sol;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IAavePool {
        function getUserAccountData(address user) external view returns (
            uint256 totalCollateralBase,
            uint256 totalDebtBase,
            uint256 availableBorrowsBase,
            uint256 currentLiquidationThreshold,
            uint256 ltv,
            uint256 healthFactor
        );

        function liquidationCall(
            address collateralAsset,
            address debtAsset,
            address user,
            uint256 debtToCover,
            bool receiveAToken
        ) external;

        event Borrow(
            address indexed reserve,
            address user,
            address indexed onBehalfOf,
            uint256 amount,
            uint8 interestRateMode,
            uint256 borrowRate,
            uint16 indexed referralCode
        );
    }
}

/// Aave V3 Pool address on Base mainnet.
pub const AAVE_V3_POOL_BASE: &str = "0xA238Dd80C259a72e81d7e4664a9801593F98d1c5";

/// Aave V3 Pool address on Base Sepolia testnet.
pub const AAVE_V3_POOL_BASE_SEPOLIA: &str = "0x07eA79F68B2B3df564D0A34F8e19D9B1e339814b";

/// Wrapped Ether on Base.
pub const WETH_BASE: &str = "0x4200000000000000000000000000000000000006";

/// USD Coin on Base.
pub const USDC_BASE: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

/// Coinbase Wrapped BTC on Base.
pub const CBBTC_BASE: &str = "0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf";
