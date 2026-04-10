use serde::Deserialize;

/// Binance bookTicker stream message.
///
/// Example payload from `<symbol>@bookTicker`:
/// ```json
/// {
///   "u": 400900217,
///   "s": "BNBUSDT",
///   "b": "25.35190000",
///   "B": "31.21000000",
///   "a": "25.36520000",
///   "A": "40.66000000"
/// }
/// ```
#[derive(Debug, Deserialize)]
pub struct BookTickerMsg {
    #[serde(rename = "u")]
    pub update_id: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "b")]
    pub bid_price: String,
    #[serde(rename = "B")]
    pub bid_qty: String,
    #[serde(rename = "a")]
    pub ask_price: String,
    #[serde(rename = "A")]
    pub ask_qty: String,
}

/// Wrapper emitted by the combined-stream endpoint.
///
/// Combined streams wrap each message as:
/// ```json
/// { "stream": "btcusdt@bookTicker", "data": { ... } }
/// ```
#[derive(Debug, Deserialize)]
pub struct CombinedStreamMsg {
    pub stream: String,
    pub data: serde_json::Value,
}

/// All message variants that the Binance WebSocket feed may produce.
#[derive(Debug)]
pub enum BinanceWsMsg {
    BookTicker(BookTickerMsg),
    /// A message type we recognise but intentionally ignore (e.g. ping frames
    /// or subscription confirmations).
    Ignored,
}
