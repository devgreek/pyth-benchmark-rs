use crate::types::{
    Bar, HistoryResponse, LastBarsCache, 
    Subscriptions, SymbolInfo, PeriodParams
};
use crate::{API_ENDPOINT};
use reqwest::Client;
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Main client for interacting with the Pyth Network API
pub struct DataFeed {
    /// Cache for the last bar of each symbol
    pub(crate) last_bars_cache: LastBarsCache,
    /// HTTP client for API requests
    pub(crate) client: Client,
    /// Active subscriptions
    pub(crate) subscriptions: Subscriptions,
}

impl DataFeed {
    /// Create a new DataFeed instance with the given subscriptions
    pub fn new(subscriptions: Subscriptions) -> Self {
        DataFeed {
            last_bars_cache: Arc::new(Mutex::new(HashMap::new())),
            client: Client::new(),
            subscriptions,
        }
    }

    /// Initialize the data feed and call the callback when ready
    /// 
    /// # Arguments
    /// * `callback` - Function to call when the data feed is ready with config data
    pub async fn on_ready<F>(&self, mut callback: F)
    where
        F: FnMut(serde_json::Value) + Send + 'static,
    {
        println!("[onReady]: Method call");
        let url = format!("{}/config", API_ENDPOINT);
        
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(config) = resp.json::<serde_json::Value>().await {
                println!("[onReady]: Config data received");
                
                // Execute callback in a separate task
                tokio::spawn(async move {
                    callback(config);
                });
            }
        }
    }

    /// Search for symbols matching the given query
    /// 
    /// # Arguments
    /// * `user_input` - Search query string
    /// * `exchange` - Exchange filter (currently unused)
    /// * `symbol_type` - Symbol type filter (currently unused)
    /// * `on_result_ready_callback` - Callback for when search results are ready
    pub async fn search_symbols<F>(
        &self,
        user_input: &str,
        _exchange: &str,
        _symbol_type: &str,
        mut on_result_ready_callback: F,
    ) where
        F: FnMut(serde_json::Value) + Send + 'static,
    {
        println!("[searchSymbols]: Method call for query: {}", user_input);
        let url = format!("{}/search?query={}", API_ENDPOINT, user_input);
        
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(data) = resp.json().await {
                on_result_ready_callback(data);
            }
        }
    }

    /// Resolve symbol information for the given symbol name
    /// 
    /// # Arguments
    /// * `symbol_name` - Name of the symbol to resolve
    /// * `on_symbol_resolved_callback` - Callback for successful resolution
    /// * `on_resolve_error_callback` - Callback for resolution errors
    pub async fn resolve_symbol<F1, F2>(
        &self,
        symbol_name: &str,
        mut on_symbol_resolved_callback: F1,
        mut on_resolve_error_callback: F2,
    ) where
        F1: FnMut(serde_json::Value) + Send + 'static,
        F2: FnMut(&str) + Send + 'static,
    {
        println!("[resolveSymbol]: Method call for {}", symbol_name);
        let url = format!("{}/symbols?symbol={}", API_ENDPOINT, symbol_name);
        
        if let Ok(resp) = self.client.get(&url).send().await {
            match resp.json().await {
                Ok(symbol_info) => {
                    println!("[resolveSymbol]: Symbol resolved");
                    on_symbol_resolved_callback(symbol_info);
                }
                Err(_) => {
                    println!("[resolveSymbol]: Cannot resolve symbol {}", symbol_name);
                    on_resolve_error_callback("Cannot resolve symbol");
                }
            }
        } else {
            on_resolve_error_callback("Failed to fetch symbol data");
        }
    }

    /// Get historical bar data for a symbol
    /// 
    /// # Arguments
    /// * `symbol_info` - Symbol information
    /// * `resolution` - Time resolution (e.g., "1D", "1H")
    /// * `period_params` - Time period parameters
    /// * `on_history_callback` - Callback for historical data
    /// * `on_error_callback` - Callback for errors
    pub async fn get_bars<F1, F2>(
        &self,
        symbol_info: &SymbolInfo,
        resolution: &str,
        period_params: &PeriodParams,
        mut on_history_callback: F1,
        _on_error_callback: F2,
    ) where
        F1: FnMut(Vec<Bar>, bool) + Send + 'static,
        F2: FnMut(anyhow::Error) + Send + 'static,
    {
        let ticker = &symbol_info.ticker;
        let (from, to, first_data_request) = (
            period_params.from,
            period_params.to,
            period_params.first_data_request,
        );

        println!("[getBars]: Method call {} {} {} {}", ticker, resolution, from, to);

        // Split large time ranges into smaller chunks (max 1 year)
        let max_range = 365 * 24 * 60 * 60;
        let mut promises = vec![];
        let mut current_from = from;

        while current_from < to {
            let current_to = std::cmp::min(to, current_from + max_range);
            let url = format!(
                "{}/history?symbol={}&resolution={}&from={}&to={}",
                API_ENDPOINT, ticker, resolution, current_from, current_to
            );
            
            promises.push(self.client.get(&url).send());
            current_from = current_to;
        }

        let mut bars: Vec<Bar> = vec![];

        // Execute all requests concurrently
        let results = futures_util::future::join_all(promises).await;
        for resp in results {
            if let Ok(r) = resp {
                if let Ok(text) = r.text().await {
                    if let Ok(data) = serde_json::from_str::<HistoryResponse>(&text) {
                        if !data.t.is_empty() {
                            for (i, &time) in data.t.iter().enumerate() {
                                bars.push(Bar {
                                    time: time * 1000, // Convert to milliseconds
                                    low: data.l[i],
                                    high: data.h[i],
                                    open: data.o[i],
                                    close: data.c[i],
                                });
                            }
                        }
                    }
                }
            }
        }

        // Cache the last bar if this is the first request
        if first_data_request && !bars.is_empty() {
            if let Ok(mut cache) = self.last_bars_cache.lock() {
                cache.insert(ticker.clone(), bars.last().unwrap().clone());
            }
        }

        let no_data = bars.is_empty();
        on_history_callback(bars, no_data);
    }

    /// Subscribe to real-time bar updates for a symbol
    /// 
    /// # Arguments
    /// * `symbol_info` - Symbol information
    /// * `resolution` - Time resolution
    /// * `on_realtime_callback` - Channel for receiving bar updates
    /// * `subscriber_uid` - Unique identifier for this subscription
    /// * `on_reset_cache_needed_callback` - Optional callback for cache resets
    pub async fn subscribe_bars(
        &self,
        symbol_info: SymbolInfo,
        resolution: String,
        on_realtime_callback: mpsc::UnboundedSender<Bar>,
        subscriber_uid: String,
        _on_reset_cache_needed_callback: Option<fn()>,
    ) {
        println!("[subscribeBars]: Method call with subscriberUID: {}", subscriber_uid);
        
        let last_bar = if let Ok(cache) = self.last_bars_cache.lock() {
            cache.get(&symbol_info.ticker).cloned()
        } else {
            None
        };

        if let Some(bar) = last_bar {
            crate::streaming::subscribe_on_stream(
                symbol_info.ticker.clone(),
                resolution,
                subscriber_uid,
                bar,
                self.subscriptions.clone(),
                on_realtime_callback,
            ).await;
        } else {
            println!("[subscribeBars]: No last bar available for {}", symbol_info.ticker);
        }
    }

    /// Unsubscribe from bar updates
    /// 
    /// # Arguments
    /// * `subscriber_uid` - Unique identifier of the subscription to remove
    pub async fn unsubscribe_bars(&self, subscriber_uid: &str) {
        println!("[unsubscribeBars]: Method call with subscriberUID: {}", subscriber_uid);
        crate::streaming::unsubscribe_from_stream(subscriber_uid, &self.subscriptions);
    }

    /// Get the cached last bar for a symbol, if available
    pub fn get_last_bar(&self, ticker: &str) -> Option<Bar> {
        self.last_bars_cache.lock().ok()?.get(ticker).cloned()
    }

    /// Manually set the last bar for a symbol in the cache
    pub fn set_last_bar(&self, ticker: String, bar: Bar) {
        if let Ok(mut cache) = self.last_bars_cache.lock() {
            cache.insert(ticker, bar);
        }
    }
}

impl Clone for DataFeed {
    fn clone(&self) -> Self {
        Self {
            last_bars_cache: self.last_bars_cache.clone(),
            client: self.client.clone(),
            subscriptions: self.subscriptions.clone(),
        }
    }
}
