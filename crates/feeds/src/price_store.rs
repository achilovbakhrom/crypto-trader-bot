use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use trader_core::types::{Price, Quote, Symbol};

#[derive(Clone, Default)]
pub struct PriceStore {
    inner: Arc<RwLock<HashMap<Symbol, Quote>>>,
}

impl PriceStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(&self, quote: Quote) {
        let mut map = self.inner.write().await;
        map.insert(quote.symbol.clone(), quote);
    }

    pub async fn get(&self, symbol: &Symbol) -> Option<Quote> {
        let map = self.inner.read().await;
        map.get(symbol).cloned()
    }

    pub async fn get_all(&self) -> Vec<Quote> {
        let map = self.inner.read().await;
        map.values().cloned().collect()
    }

    /// Returns best ask for a symbol (what you pay to buy)
    pub async fn ask(&self, symbol: &Symbol) -> Option<Price> {
        let map = self.inner.read().await;
        map.get(symbol).map(|q| q.ask)
    }

    /// Returns best bid for a symbol (what you receive when selling)
    pub async fn bid(&self, symbol: &Symbol) -> Option<Price> {
        let map = self.inner.read().await;
        map.get(symbol).map(|q| q.bid)
    }
}
