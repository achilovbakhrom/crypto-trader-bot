use crate::error::CoreError;
use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub app: AppSettings,
    pub chain: ChainSettings,
    pub binance: BinanceSettings,
    pub liquidation: LiquidationSettings,
    pub token_unlock: TokenUnlockSettings,
    pub database: DatabaseSettings,
    pub observability: ObservabilitySettings,
    pub web: WebSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppSettings {
    pub name: String,
    pub log_level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChainSettings {
    pub network: String,
    pub rpc_url: String,
    pub ws_url: String,
    pub poll_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BinanceSettings {
    pub rest_url: String,
    pub ws_url: String,
    pub recv_window: u64,
    #[serde(skip)]
    pub api_key: String,
    #[serde(skip)]
    pub api_secret: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LiquidationSettings {
    pub enabled: bool,
    pub min_profit_usd_cents: i64,
    pub max_position_usd_cents: i64,
    pub health_factor_threshold: String,
    pub protocols: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TokenUnlockSettings {
    pub enabled: bool,
    pub min_unlock_usd: i64,
    pub days_before_entry: u32,
    pub days_before_exit: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ObservabilitySettings {
    pub openobserve_url: String,
    pub openobserve_token: String,
    pub service_name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebSettings {
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn load() -> Result<Self, CoreError> {
        // Load .env file if present (non-fatal if missing)
        let _ = dotenvy::dotenv();

        let cfg = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(Environment::default().separator("_").ignore_empty(true))
            .build()?;

        let mut app: AppConfig = cfg.try_deserialize()?;

        // Load secrets from environment (never from config files)
        app.binance.api_key = std::env::var("BINANCE_API_KEY")
            .map_err(|_| CoreError::EnvVar("BINANCE_API_KEY".into()))?;
        app.binance.api_secret = std::env::var("BINANCE_API_SECRET")
            .map_err(|_| CoreError::EnvVar("BINANCE_API_SECRET".into()))?;
        app.chain.rpc_url = std::env::var("CHAIN_RPC_URL")
            .map_err(|_| CoreError::EnvVar("CHAIN_RPC_URL".into()))?;
        app.chain.ws_url = std::env::var("CHAIN_WS_URL").unwrap_or_else(|_| {
            app.chain
                .rpc_url
                .replace("https://", "wss://")
                .replace("http://", "ws://")
        });
        app.database.url =
            std::env::var("DATABASE_URL").map_err(|_| CoreError::EnvVar("DATABASE_URL".into()))?;

        Ok(app)
    }
}
