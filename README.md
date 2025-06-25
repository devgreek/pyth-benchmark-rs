# Pyth Benchmark Rust Client

[![Crates.io](https://img.shields.io/crates/v/pyth-benchmark-rs.svg)](https://crates.io/crates/pyth-benchmark-rs)
[![Documentation](https://docs.rs/pyth-benchmark-rs/badge.svg)](https://docs.rs/pyth-benchmark-rs)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/yourusername/pyth-benchmark-rs)

A Rust client for streaming and processing price data from the Pyth Network oracle. This crate provides a robust interface for accessing real-time price data and converting it into OHLC (Open, High, Low, Close) bar data.

## 🚀 Features

- **Real-time Price Streaming**: Connect to Pyth Network's streaming API for live price updates
- **OHLC Bar Generation**: Automatically convert price feeds into candlestick bar data
- **Daily Bar Aggregation**: Aggregate price data into daily time periods
- **Subscription Management**: Thread-safe subscription system for multiple symbols
- **Automatic Reconnection**: Resilient connection handling with exponential backoff
- **Multiple Data Formats**: Support for JSON and Server-Sent Events (SSE)

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
pyth-benchmark-rs = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## 🏃 Quick Start

```rust
use pyth_benchmark_rs::{DataFeed, SymbolInfo, PeriodParams, start_streaming};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize subscriptions registry
    let subscriptions = Arc::new(Mutex::new(HashMap::new()));
    
    // Create DataFeed instance
    let datafeed = DataFeed::new(subscriptions.clone());
    
    // Initialize the data feed
    datafeed.on_ready(|config| {
        println!("DataFeed ready: {:?}", config);
    }).await;
    
    // Start streaming in background
    let subs_clone = subscriptions.clone();
    tokio::spawn(async move {
        let _ = start_streaming(subs_clone).await;
    });
    
    // Get historical data
    let symbol = SymbolInfo::new("Crypto.BTC/USD");
    let period = PeriodParams::last_days(30, true);
    
    datafeed.get_bars(
        &symbol,
        "1D",
        &period,
        |bars, no_data| {
            if !no_data {
                println!("Received {} historical bars", bars.len());
                for bar in bars.iter().take(3) {
                    println!("Bar: O:{:.2} H:{:.2} L:{:.2} C:{:.2}", 
                        bar.open, bar.high, bar.low, bar.close);
                }
            }
        },
        |err| eprintln!("Error: {}", err)
    ).await;
    
    Ok(())
}
```

## 📊 Real-time Subscriptions

Subscribe to real-time price updates:

```rust
use tokio::sync::mpsc;

// Create a channel for receiving updates
let (tx, mut rx) = mpsc::unbounded_channel();

// Subscribe to BTC/USD daily bars
let symbol = SymbolInfo::new("Crypto.BTC/USD");
datafeed.subscribe_bars(
    symbol,
    "1D".to_string(),
    tx,
    "my_subscription".to_string(),
    None
).await;

// Listen for updates
tokio::spawn(async move {
    while let Some(bar) = rx.recv().await {
        println!("New bar: O:{:.2} H:{:.2} L:{:.2} C:{:.2} T:{}", 
            bar.open, bar.high, bar.low, bar.close, bar.time);
    }
});
```

## 🏗️ API Overview

### Core Types

- **`DataFeed`**: Main client for interacting with Pyth Network
- **`Bar`**: OHLC price data structure
- **`SymbolInfo`**: Symbol information (ticker)
- **`PeriodParams`**: Time period parameters for historical data

### Key Methods

- **`on_ready()`**: Initialize the data feed
- **`get_bars()`**: Fetch historical OHLC data
- **`subscribe_bars()`**: Subscribe to real-time updates
- **`search_symbols()`**: Search for available symbols
- **`resolve_symbol()`**: Get detailed symbol information

### Streaming Functions

- **`start_streaming()`**: Start the main streaming connection
- **`subscribe_on_stream()`**: Add a symbol subscription
- **`unsubscribe_from_stream()`**: Remove a subscription

## 🎯 What This Crate is Built For

This repository is specifically designed to **read OHLC (Open, High, Low, Close) data from the Pyth Network oracle**. Pyth Network is a decentralized oracle that provides real-time market data for various financial instruments, particularly in the cryptocurrency space.

### Use Cases

- **Trading Applications**: Build real-time trading bots and algorithms
- **Market Analysis**: Analyze price movements and trends
- **Data Collection**: Store historical price data for research
- **Charting Applications**: Create price charts and technical indicators
- **Portfolio Management**: Track asset prices for portfolio valuation

## 🔧 Configuration

The crate uses these default endpoints:

- **Streaming**: `https://benchmarks.pyth.network/v1/shims/tradingview/streaming`
- **API**: `https://benchmarks.pyth.network/v1/shims/tradingview`

## 🛠️ Building and Running

Build the library:
```bash
cargo build
```

Run the example:
```bash
cargo run --bin example
```

Run tests:
```bash
cargo test
```

## 📖 Examples

The `src/main.rs` file contains a comprehensive example showing:

1. Fetching historical data
2. Starting real-time streaming
3. Subscribing to symbol updates
4. Handling bar data updates
5. Graceful shutdown

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## 📄 License

This project is licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🔗 Links

- [Pyth Network](https://pyth.network/)
- [Pyth Network Documentation](https://docs.pyth.network/)
- [Crates.io](https://crates.io/crates/pyth-benchmark-rs)
- [Documentation](https://docs.rs/pyth-benchmark-rs)
