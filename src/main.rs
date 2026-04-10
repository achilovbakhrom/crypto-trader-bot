use anyhow::Context;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Config ────────────────────────────────────────────────────────────────
    let cfg = trader_core::config::AppConfig::load()
        .context("Failed to load configuration")?;

    // ── Telemetry ─────────────────────────────────────────────────────────────
    let _telemetry_guard = monitor::telemetry::init_telemetry(
        &cfg.observability.service_name,
        &cfg.observability.openobserve_url,
        &cfg.observability.openobserve_token,
        &cfg.app.log_level,
    )
    .context("Failed to initialize telemetry")?;

    info!(
        service = %cfg.app.name,
        network = %cfg.chain.network,
        "Starting crypto-trader-bot"
    );

    // ── Database ──────────────────────────────────────────────────────────────
    let db = storage::db::Database::connect(
        &cfg.database.url,
        cfg.database.max_connections,
        cfg.database.min_connections,
    )
    .await
    .context("Failed to connect to database")?;

    db.run_migrations()
        .await
        .context("Failed to run database migrations")?;

    info!("Database connected and migrations applied");
    let db = Arc::new(db);

    // ── Metrics ───────────────────────────────────────────────────────────────
    let metrics = Arc::new(monitor::metrics::Metrics::new());

    // ── Price Store (shared between strategies and TUI) ───────────────────────
    let price_store = Arc::new(feeds::price_store::PriceStore::new());

    // ── Binance Price Feed ────────────────────────────────────────────────────
    let symbols = vec![
        "BTCUSDT".to_string(),
        "ETHUSDT".to_string(),
        "USDCUSDT".to_string(),
    ];
    let binance_feed = feeds::binance::BinanceFeed::new(
        cfg.binance.ws_url.clone(),
        symbols,
        (*price_store).clone(),
    );

    // ── Binance Execution Client ──────────────────────────────────────────────
    let binance_client = Arc::new(execution::binance::BinanceClient::new(
        cfg.binance.rest_url.clone(),
        cfg.binance.api_key.clone(),
        cfg.binance.api_secret.clone(),
        cfg.binance.recv_window,
    ));

    // ── Chain Monitor (Aave V3 on Base) ───────────────────────────────────────
    let pool_address = if cfg.chain.network.contains("sepolia") {
        chain::aave::types::AAVE_V3_POOL_BASE_SEPOLIA
    } else {
        chain::aave::types::AAVE_V3_POOL_BASE
    };

    let aave_monitor = Arc::new(
        chain::aave::AaveMonitor::new(
            &cfg.chain.rpc_url,
            pool_address,
            cfg.chain.poll_interval_ms,
        )
        .await
        .context("Failed to initialize Aave monitor")?,
    );

    // ── Liquidation Strategy ──────────────────────────────────────────────────
    let private_key = std::env::var("PRIVATE_KEY")
        .context("PRIVATE_KEY environment variable not set")?;

    let (opportunity_tx, opportunity_rx) = mpsc::channel(100);

    let liquidation_executor = Arc::new(liquidation::executor::LiquidationExecutor::new(
        binance_client.clone(),
        private_key,
        pool_address.to_string(),
    ));

    let liquidation_detector = liquidation::detector::LiquidationDetector::new(
        cfg.liquidation.min_profit_usd_cents,
        cfg.liquidation.max_position_usd_cents,
    );

    let liquidation_strategy = Arc::new(liquidation::strategy::LiquidationStrategy::new(
        aave_monitor.clone(),
        liquidation_detector,
        liquidation_executor,
        db.clone(),
        metrics.clone(),
        price_store.clone(),
        cfg.liquidation.min_profit_usd_cents,
    ));

    // ── Web Server State ──────────────────────────────────────────────────────
    let web_state = web_ui::state::AppState::new(db.clone(), metrics.clone());
    let web_server = web_ui::server::WebServer::new(
        web_state,
        cfg.web.host.clone(),
        cfg.web.port,
    );

    // ── TUI ───────────────────────────────────────────────────────────────────
    let (tui_app, tui_event_rx) = tui_ui::app::App::new();

    // ── Chain provider for strategy execution ─────────────────────────────────
    let provider = Arc::new(
        chain::provider::create_http_provider(&cfg.chain.rpc_url)
            .await
            .context("Failed to create chain provider")?,
    );

    // ── Spawn all tasks ───────────────────────────────────────────────────────
    info!("All components initialized — starting tasks");

    let result = tokio::select! {
        // Price feed (reconnects automatically on disconnect)
        r = binance_feed.run() => {
            error!("Binance feed exited: {:?}", r);
            r.map_err(anyhow::Error::from)
        }

        // Aave health factor monitor → sends opportunities to channel
        r = aave_monitor.run(opportunity_tx) => {
            error!("Aave monitor exited: {:?}", r);
            r.map_err(anyhow::Error::from)
        }

        // Liquidation strategy loop
        r = liquidation_strategy.run(opportunity_rx, provider) => {
            error!("Liquidation strategy exited: {:?}", r);
            r
        }

        // Web API server
        r = web_server.run() => {
            error!("Web server exited: {:?}", r);
            r
        }

        // Terminal UI (blocks until user presses 'q')
        r = tui_app.run(tui_event_rx) => {
            info!("TUI exited — shutting down");
            r.map_err(anyhow::Error::from)
        }
    };

    info!("Shutdown complete");
    result
}
