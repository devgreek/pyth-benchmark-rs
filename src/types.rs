use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// A single OHLC bar representing price data for a specific time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    /// Timestamp in milliseconds since Unix epoch
    pub time: u64,
    /// Lowest price during the period
    pub low: f64,
    /// Highest price during the period
    pub high: f64,
    /// Opening price of the period
    pub open: f64,
    /// Closing price of the period
    pub close: f64,
}

/// Handler for managing subscription callbacks
#[derive(Clone)]
pub struct Handler {
    /// Unique identifier for this handler
    pub id: String,
    /// Channel sender for streaming bar updates
    pub callback: mpsc::UnboundedSender<Bar>,
}

/// A subscription to a specific symbol and resolution
#[derive(Clone)]
pub struct Subscription {
    /// Unique identifier for the subscriber
    pub subscriber_uid: String,
    /// Time resolution (e.g., "1D", "1H", "15m")
    pub resolution: String,
    /// The last daily bar for this subscription
    pub last_daily_bar: Bar,
    /// List of handlers receiving updates for this subscription
    pub handlers: Vec<Handler>,
}

/// Thread-safe collection of active subscriptions
pub type Subscriptions = Arc<Mutex<HashMap<String, Subscription>>>;

/// Thread-safe cache for the last bars of each symbol
pub type LastBarsCache = Arc<Mutex<HashMap<String, Bar>>>;

/// Information about a trading symbol
#[derive(Clone, Debug)]
pub struct SymbolInfo {
    /// The symbol ticker (e.g., "BTC/USD", "Crypto.ETH/USD")
    pub ticker: String,
}

/// Parameters for requesting historical bar data
#[derive(Clone, Debug)]
pub struct PeriodParams {
    /// Start timestamp in seconds since Unix epoch
    pub from: u64,
    /// End timestamp in seconds since Unix epoch
    pub to: u64,
    /// Whether this is the first data request for this symbol
    pub first_data_request: bool,
}

/// Streaming data message format from Pyth Network
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct StreamData {
    /// Symbol identifier
    pub id: String,
    /// Price value
    pub p: f64,
    /// Timestamp
    pub t: u64,
}

/// Different types of messages that can come from the stream
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub(crate) enum StreamMessage {
    /// Actual price data
    Data(StreamData),
    /// Heartbeat or numeric ID messages
    HeartbeatOrId(i64),
    /// Any other JSON format
    Other(serde_json::Value),
}

/// Response format for historical data from the API
#[derive(Deserialize, Debug)]
pub(crate) struct HistoryResponse {
    /// Array of timestamps
    pub t: Vec<u64>,
    /// Array of low prices
    pub l: Vec<f64>,
    /// Array of high prices
    pub h: Vec<f64>,
    /// Array of opening prices
    pub o: Vec<f64>,
    /// Array of closing prices
    pub c: Vec<f64>,
}

impl Bar {
    /// Create a new bar with the given parameters
    pub fn new(time: u64, open: f64, high: f64, low: f64, close: f64) -> Self {
        Self {
            time,
            open,
            high,
            low,
            close,
        }
    }

    /// Update this bar with a new price, adjusting high/low/close as needed
    pub fn update_with_price(&mut self, price: f64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
    }
}

impl SymbolInfo {
    /// Create a new SymbolInfo with the given ticker
    pub fn new(ticker: impl Into<String>) -> Self {
        Self {
            ticker: ticker.into(),
        }
    }
}

impl PeriodParams {
    /// Create new period parameters
    pub fn new(from: u64, to: u64, first_data_request: bool) -> Self {
        Self {
            from,
            to,
            first_data_request,
        }
    }

    /// Create parameters for the last N days
    pub fn last_days(days: u64, first_data_request: bool) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            from: now - (days * 24 * 60 * 60),
            to: now,
            first_data_request,
        }
    }
}
