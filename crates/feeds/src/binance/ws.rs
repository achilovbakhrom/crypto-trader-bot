use std::time::Duration;

use futures::StreamExt;
use rust_decimal::Decimal;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

use trader_core::types::{Amount, Exchange, Price, Quote, Symbol};

use crate::error::FeedsError;
use crate::price_store::PriceStore;

use super::types::{BookTickerMsg, CombinedStreamMsg};

pub struct BinanceFeed {
    ws_url: String,
    symbols: Vec<String>,
    price_store: PriceStore,
}

impl BinanceFeed {
    pub fn new(ws_url: String, symbols: Vec<String>, price_store: PriceStore) -> Self {
        Self {
            ws_url,
            symbols,
            price_store,
        }
    }

    /// Build the combined-stream URL.
    ///
    /// Format:
    /// `<base_url>/stream?streams=btcusdt@bookTicker/ethusdt@bookTicker`
    fn stream_url(&self) -> String {
        let streams = self
            .symbols
            .iter()
            .map(|s| format!("{}@bookTicker", s.to_lowercase()))
            .collect::<Vec<_>>()
            .join("/");

        format!("{}/stream?streams={}", self.ws_url.trim_end_matches('/'), streams)
    }

    /// Connect and start streaming. Reconnects automatically on disconnect.
    /// Returns only on a fatal, non-recoverable error.
    pub async fn run(&self) -> Result<(), FeedsError> {
        loop {
            match self.connect_and_stream().await {
                Ok(()) => {
                    // clean close – treat as disconnect and reconnect
                    warn!("Binance WebSocket closed cleanly; reconnecting in 2 s");
                }
                Err(FeedsError::Disconnected) => {
                    warn!("Binance WebSocket disconnected; reconnecting in 2 s");
                }
                Err(FeedsError::ConnectionFailed(ref msg)) => {
                    warn!(
                        error = %msg,
                        "Binance WebSocket connection failed; reconnecting in 2 s"
                    );
                }
                Err(FeedsError::WebSocketError(ref msg)) => {
                    warn!(
                        error = %msg,
                        "Binance WebSocket error; reconnecting in 2 s"
                    );
                }
                Err(e) => {
                    // ParseError is non-recoverable in the sense that we
                    // still want to keep running – log and reconnect.
                    error!(error = %e, "Unexpected feeds error; reconnecting in 2 s");
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    async fn connect_and_stream(&self) -> Result<(), FeedsError> {
        let url = self.stream_url();
        info!(url = %url, "Connecting to Binance WebSocket");

        let (ws_stream, _response) = connect_async(&url).await.map_err(|e| {
            FeedsError::ConnectionFailed(format!("failed to connect to {url}: {e}"))
        })?;

        info!(url = %url, "Connected to Binance WebSocket");

        let (_sink, mut stream) = ws_stream.split();

        while let Some(msg_result) = stream.next().await {
            let raw = match msg_result {
                Ok(Message::Text(text)) => text.to_string(),
                Ok(Message::Binary(bytes)) => {
                    // Some Binance endpoints send binary text frames.
                    match String::from_utf8(bytes.to_vec()) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!(error = %e, "Received non-UTF-8 binary frame; skipping");
                            continue;
                        }
                    }
                }
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                    // tungstenite handles pong replies automatically.
                    continue;
                }
                Ok(Message::Close(frame)) => {
                    info!(frame = ?frame, "Received WebSocket close frame");
                    return Err(FeedsError::Disconnected);
                }
                Ok(_) => continue,
                Err(e) => {
                    return Err(FeedsError::WebSocketError(e.to_string()));
                }
            };

            if let Some(quote) = self.parse_and_update(raw.as_str()) {
                self.price_store.update(quote).await;
            }
        }

        // Stream ended without a close frame.
        Err(FeedsError::Disconnected)
    }

    /// Parse a raw JSON text frame and return a `Quote` on success.
    ///
    /// Returns `None` when the message is not a book-ticker (e.g. subscription
    /// confirmation) so the caller can silently skip it.
    fn parse_and_update(&self, msg: &str) -> Option<Quote> {
        // Combined-stream envelope: {"stream":"…","data":{…}}
        let parsed = serde_json::from_str::<CombinedStreamMsg>(msg).ok();

        let ticker: BookTickerMsg = if let Some(envelope) = parsed {
            serde_json::from_value(envelope.data)
                .map_err(|e| {
                    warn!(error = %e, raw = %msg, "Failed to parse bookTicker data");
                })
                .ok()?
        } else {
            // Try a bare bookTicker message (single-stream endpoint).
            serde_json::from_str::<BookTickerMsg>(msg)
                .map_err(|e| {
                    warn!(error = %e, raw = %msg, "Failed to parse WebSocket message");
                })
                .ok()?
        };

        let bid_price = ticker.bid_price.parse::<Decimal>().map_err(|e| {
            warn!(error = %e, raw = %ticker.bid_price, "Failed to parse bid_price");
        }).ok()?;

        let bid_qty = ticker.bid_qty.parse::<Decimal>().map_err(|e| {
            warn!(error = %e, raw = %ticker.bid_qty, "Failed to parse bid_qty");
        }).ok()?;

        let ask_price = ticker.ask_price.parse::<Decimal>().map_err(|e| {
            warn!(error = %e, raw = %ticker.ask_price, "Failed to parse ask_price");
        }).ok()?;

        let ask_qty = ticker.ask_qty.parse::<Decimal>().map_err(|e| {
            warn!(error = %e, raw = %ticker.ask_qty, "Failed to parse ask_qty");
        }).ok()?;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let quote = Quote {
            symbol: Symbol::new(ticker.symbol.to_uppercase()),
            exchange: Exchange::Binance,
            bid: Price(bid_price),
            bid_qty: Amount(bid_qty),
            ask: Price(ask_price),
            ask_qty: Amount(ask_qty),
            timestamp_ms,
        };

        info!(
            symbol = %quote.symbol,
            bid  = %bid_price,
            ask  = %ask_price,
            "Quote updated"
        );

        Some(quote)
    }
}
