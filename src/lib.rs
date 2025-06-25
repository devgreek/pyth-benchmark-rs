//! # Pyth Benchmark Rust Client
//!
//! A Rust client for streaming and processing price data from the Pyth Network oracle.
//! This crate provides a robust interface for accessing real-time price data and
//! converting it into OHLC (Open, High, Low, Close) bar data.
//!
//! ## Features
//!
//! - Real-time price streaming from Pyth Network
//! - OHLC bar generation from price feeds
//! - Daily bar aggregation
//! - Subscription-based architecture
//! - Automatic reconnection with exponential backoff
//! - Thread-safe subscription management
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use pyth_benchmark_rs::{DataFeed, SymbolInfo, PeriodParams, start_streaming};
//! use std::sync::{Arc, Mutex};
//! use std::collections::HashMap;
//! use tokio::sync::mpsc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Initialize subscriptions
//!     let subscriptions = Arc::new(Mutex::new(HashMap::new()));
//!     
//!     // Create DataFeed instance
//!     let datafeed = DataFeed::new(subscriptions.clone());
//!     
//!     // Start streaming in background
//!     let subs_clone = subscriptions.clone();
//!     tokio::spawn(async move {
//!         let _ = start_streaming(subs_clone).await;
//!     });
//!     
//!     // Use the datafeed...
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod streaming;
pub mod types;

pub use client::DataFeed;
pub use streaming::{start_streaming, subscribe_on_stream, unsubscribe_from_stream};
pub use types::{Bar, Handler, Subscription, Subscriptions, SymbolInfo, PeriodParams};

/// The default Pyth Network streaming URL
pub const STREAMING_URL: &str = "https://benchmarks.pyth.network/v1/shims/tradingview/streaming";

/// The default Pyth Network API endpoint
pub const API_ENDPOINT: &str = "https://benchmarks.pyth.network/v1/shims/tradingview";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_bar_creation() {
        let bar = Bar {
            time: 1_717_000_000,
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
        };
        
        assert_eq!(bar.time, 1_717_000_000);
        assert_eq!(bar.open, 100.0);
        assert_eq!(bar.high, 105.0);
        assert_eq!(bar.low, 95.0);
        assert_eq!(bar.close, 102.0);
    }

    #[test]
    fn test_symbol_info_creation() {
        let symbol = SymbolInfo {
            ticker: "BTC/USD".to_string(),
        };
        
        assert_eq!(symbol.ticker, "BTC/USD");
    }

    #[test]
    fn test_datafeed_creation() {
        let subscriptions = Arc::new(Mutex::new(HashMap::new()));
        let datafeed = DataFeed::new(subscriptions);
        
        // Basic test to ensure DataFeed can be created
        assert!(format!("{:?}", datafeed.client).len() > 0);
    }
}
