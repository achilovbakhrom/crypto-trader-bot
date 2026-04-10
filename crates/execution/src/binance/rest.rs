use chrono::Utc;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use serde::Deserialize;
use sha2::Sha256;
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::error::ExecutionError;
use crate::order::{Balance, Fill, OrderResult, OrderStatus, Side};

type HmacSha256 = Hmac<Sha256>;

/// Raw Binance order response shape (camelCase from the wire).
#[derive(Debug, Deserialize)]
struct BinanceOrderResponse {
    #[serde(rename = "orderId")]
    order_id: u64,
    symbol: String,
    side: String,
    status: String,
    #[serde(rename = "executedQty")]
    executed_qty: String,
    #[serde(rename = "cummulativeQuoteQty")]
    cummulative_quote_qty: String,
    #[serde(default)]
    fills: Vec<BinanceFill>,
}

#[derive(Debug, Deserialize)]
struct BinanceFill {
    price: String,
    qty: String,
    commission: String,
    #[serde(rename = "commissionAsset")]
    commission_asset: String,
}

/// Raw Binance API error body.
#[derive(Debug, Deserialize)]
struct BinanceApiError {
    code: i32,
    msg: String,
}

/// Raw Binance account info response (only the parts we care about).
#[derive(Debug, Deserialize)]
struct BinanceAccountResponse {
    balances: Vec<BinanceBalance>,
}

#[derive(Debug, Deserialize)]
struct BinanceBalance {
    asset: String,
    free: String,
    locked: String,
}

// ---------------------------------------------------------------------------

pub struct BinanceClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    api_secret: String,
    recv_window: u64,
}

impl BinanceClient {
    pub fn new(base_url: String, api_key: String, api_secret: String, recv_window: u64) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");

        Self {
            http,
            base_url,
            api_key,
            api_secret,
            recv_window,
        }
    }

    // -----------------------------------------------------------------------
    // Public order methods
    // -----------------------------------------------------------------------

    /// Place a market buy order. Returns the full order result.
    pub async fn market_buy(
        &self,
        symbol: &str,
        quantity: Decimal,
    ) -> Result<OrderResult, ExecutionError> {
        info!(symbol, %quantity, "placing market BUY order");

        let params = vec![
            ("symbol", symbol.to_uppercase()),
            ("side", "BUY".to_string()),
            ("type", "MARKET".to_string()),
            ("quantity", quantity.to_string()),
        ];

        let response = self.signed_post("/api/v3/order", params).await?;
        self.parse_order_response(response).await
    }

    /// Place a market sell order. Returns the full order result.
    pub async fn market_sell(
        &self,
        symbol: &str,
        quantity: Decimal,
    ) -> Result<OrderResult, ExecutionError> {
        info!(symbol, %quantity, "placing market SELL order");

        let params = vec![
            ("symbol", symbol.to_uppercase()),
            ("side", "SELL".to_string()),
            ("type", "MARKET".to_string()),
            ("quantity", quantity.to_string()),
        ];

        let response = self.signed_post("/api/v3/order", params).await?;
        self.parse_order_response(response).await
    }

    // -----------------------------------------------------------------------
    // Account / balance methods
    // -----------------------------------------------------------------------

    /// Get all account balances. Returns only assets with non-zero free or locked.
    pub async fn get_balances(&self) -> Result<Vec<Balance>, ExecutionError> {
        debug!("fetching account balances");

        let timestamp = Utc::now().timestamp_millis().to_string();
        let params = vec![
            ("recvWindow", self.recv_window.to_string()),
            ("timestamp", timestamp),
        ];

        let query_string = build_query_string(&params);
        let signature = self.sign(&query_string);
        let url = format!(
            "{}/api/v3/account?{}&signature={}",
            self.base_url, query_string, signature
        );

        debug!(url = %url, "GET account");

        let response = self
            .http
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .map_err(|e| ExecutionError::HttpError(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExecutionError::HttpError(e.to_string()))?;

        debug!(status = %status, body = %body, "GET account response");

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!("rate limited by Binance");
            return Err(ExecutionError::RateLimited);
        }

        if !status.is_success() {
            let api_err: BinanceApiError = serde_json::from_str(&body)
                .map_err(|_| ExecutionError::HttpError(format!("HTTP {status}: {body}")))?;
            return Err(ExecutionError::ApiError {
                code: api_err.code,
                msg: api_err.msg,
            });
        }

        let account: BinanceAccountResponse = serde_json::from_str(&body)?;

        let balances = account
            .balances
            .into_iter()
            .filter_map(|b| {
                let free = Decimal::from_str(&b.free).ok()?;
                let locked = Decimal::from_str(&b.locked).ok()?;
                if free.is_zero() && locked.is_zero() {
                    return None;
                }
                Some(Balance {
                    asset: b.asset,
                    free,
                    locked,
                })
            })
            .collect();

        Ok(balances)
    }

    /// Get balance for a specific asset. Returns an error if the asset is not found.
    pub async fn get_balance(&self, asset: &str) -> Result<Balance, ExecutionError> {
        let balances = self.get_balances().await?;
        let asset_upper = asset.to_uppercase();

        balances
            .into_iter()
            .find(|b| b.asset == asset_upper)
            .ok_or_else(|| ExecutionError::InsufficientBalance {
                asset: asset_upper,
                needed: "any".to_string(),
                available: "0".to_string(),
            })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build HMAC-SHA256 signature for Binance API.
    fn sign(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take a key of any length");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Generic signed POST to a Binance endpoint.
    ///
    /// Adds `timestamp` and `recvWindow`, builds the query string, appends the
    /// HMAC-SHA256 signature, then POSTs with the API key header.
    async fn signed_post(
        &self,
        path: &str,
        mut params: Vec<(&str, String)>,
    ) -> Result<reqwest::Response, ExecutionError> {
        let timestamp = Utc::now().timestamp_millis().to_string();
        params.push(("timestamp", timestamp));
        params.push(("recvWindow", self.recv_window.to_string()));

        // Build the query string from all params (without signature yet).
        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let signature = self.sign(&query_string);
        let full_query = format!("{}&signature={}", query_string, signature);

        let url = format!("{}{}", self.base_url, path);
        debug!(url = %url, query = %query_string, "signed POST");

        let response = self
            .http
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(full_query)
            .send()
            .await
            .map_err(|e| ExecutionError::HttpError(e.to_string()))?;

        Ok(response)
    }

    /// Consume a `reqwest::Response` and parse it into an `OrderResult`.
    async fn parse_order_response(
        &self,
        response: reqwest::Response,
    ) -> Result<OrderResult, ExecutionError> {
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ExecutionError::HttpError(e.to_string()))?;

        debug!(http_status = %status, body = %body, "order response body");

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!("rate limited by Binance");
            return Err(ExecutionError::RateLimited);
        }

        if !status.is_success() {
            let api_err: BinanceApiError = serde_json::from_str(&body)
                .map_err(|_| ExecutionError::HttpError(format!("HTTP {status}: {body}")))?;

            // -2010 = insufficient balance on Binance
            if api_err.code == -2010 {
                return Err(ExecutionError::InsufficientBalance {
                    asset: String::new(),
                    needed: String::new(),
                    available: String::new(),
                });
            }

            return Err(ExecutionError::ApiError {
                code: api_err.code,
                msg: api_err.msg,
            });
        }

        let raw: BinanceOrderResponse = serde_json::from_str(&body)?;

        let side = parse_side(&raw.side)?;
        let order_status = parse_order_status(&raw.status)?;

        let executed_qty = Decimal::from_str(&raw.executed_qty)
            .map_err(|e| ExecutionError::HttpError(format!("bad executedQty: {e}")))?;

        let cummulative_quote_qty = Decimal::from_str(&raw.cummulative_quote_qty)
            .map_err(|e| ExecutionError::HttpError(format!("bad cummulativeQuoteQty: {e}")))?;

        let fills = raw
            .fills
            .into_iter()
            .map(|f| {
                let price = Decimal::from_str(&f.price)
                    .map_err(|e| ExecutionError::HttpError(format!("bad fill price: {e}")))?;
                let qty = Decimal::from_str(&f.qty)
                    .map_err(|e| ExecutionError::HttpError(format!("bad fill qty: {e}")))?;
                let commission = Decimal::from_str(&f.commission)
                    .map_err(|e| ExecutionError::HttpError(format!("bad fill commission: {e}")))?;
                Ok(Fill {
                    price,
                    qty,
                    commission,
                    commission_asset: f.commission_asset,
                })
            })
            .collect::<Result<Vec<Fill>, ExecutionError>>()?;

        let result = OrderResult {
            order_id: raw.order_id,
            symbol: raw.symbol.clone(),
            side,
            status: order_status,
            executed_qty,
            cummulative_quote_qty,
            fills,
        };

        info!(
            order_id = result.order_id,
            symbol = %result.symbol,
            side = ?result.side,
            status = ?result.status,
            executed_qty = %result.executed_qty,
            "order placed successfully"
        );

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_query_string(params: &[(&str, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

fn parse_side(s: &str) -> Result<Side, ExecutionError> {
    match s {
        "BUY" => Ok(Side::Buy),
        "SELL" => Ok(Side::Sell),
        other => Err(ExecutionError::HttpError(format!(
            "unknown side from Binance: {other}"
        ))),
    }
}

fn parse_order_status(s: &str) -> Result<OrderStatus, ExecutionError> {
    match s {
        "NEW" => Ok(OrderStatus::New),
        "PARTIALLY_FILLED" => Ok(OrderStatus::PartiallyFilled),
        "FILLED" => Ok(OrderStatus::Filled),
        "CANCELED" => Ok(OrderStatus::Canceled),
        "REJECTED" => Ok(OrderStatus::Rejected),
        "EXPIRED" => Ok(OrderStatus::Expired),
        other => Err(ExecutionError::HttpError(format!(
            "unknown order status from Binance: {other}"
        ))),
    }
}
