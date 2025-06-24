# Pyth Benchmark Rust Client

A Rust-based client for streaming and processing price data from the Pyth Network oracle. This client connects to Pyth's benchmarks API to receive real-time price updates and converts them into OHLC (Open, High, Low, Close) bar data.

## Overview

This repository provides a robust and efficient implementation for accessing Pyth Network oracle data in Rust. It features:

- Real-time price streaming from Pyth Network
- OHLC (candlestick) bar generation from price feeds
- Daily bar aggregation
- Subscription-based architecture for easy integration
- Resilient connection handling with automatic reconnection
- Support for multiple data formats (JSON, SSE)

## How It Works

The client establishes a persistent connection to Pyth Network's streaming API and processes incoming price data. It organizes this data into time-based OHLC bars (candlesticks) that can be used for:

- Market data analysis
- Price charting
- Trading algorithms
- Historical data collection

## Features

- **Robust Connection Handling**: Implements automatic reconnection with exponential backoff
- **Efficient Stream Processing**: Handles chunked data and partial messages
- **Thread-safe Subscription Management**: Uses Rust's concurrency primitives for safe access
- **Multiple Message Format Support**: Processes both standard JSON and Server-Sent Events (SSE)
- **Daily Bar Aggregation**: Automatically creates and updates daily price bars

## Usage Example

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the subscription registry
    let subscriptions: Subscriptions = Arc::new(Mutex::new(HashMap::new()));
    
    // Start the streaming service in a separate task
    let subs_clone = subscriptions.clone();
    tokio::spawn(async move {
        let _ = start_streaming(subs_clone).await;
    });

    // Create a channel to receive bar updates
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Subscribe to BTC/USD with daily resolution
    let last_daily_bar = Bar {
        time: 1_717_000_000,
        open: 100.0,
        high: 100.0,
        low: 100.0,
        close: 100.0,
    };
    
    subscribe_on_stream(
        "BTC/USD".to_string(),
        "1D".to_string(),
        "sub1".to_string(),
        last_daily_bar,
        subscriptions.clone(),
        tx
    ).await;

    // Process incoming bar updates
    tokio::spawn(async move {
        while let Some(bar) = rx.recv().await {
            println!("Received bar: {:?}", bar);
            // Process the bar data as needed
        }
    });

    // Keep the application running until Ctrl+C
    tokio::signal::ctrl_c().await?;
    Ok(())
}

I'll create a README.md file for your repository that explains the purpose, functionality, and usage of your Pyth benchmark client in Rust.

```markdown
# Pyth Benchmark Rust Client

A Rust-based client for streaming and processing price data from the Pyth Network oracle. This client connects to Pyth's benchmarks API to receive real-time price updates and converts them into OHLC (Open, High, Low, Close) bar data.

## Overview

This repository provides a robust and efficient implementation for accessing Pyth Network oracle data in Rust. It features:

- Real-time price streaming from Pyth Network
- OHLC (candlestick) bar generation from price feeds
- Daily bar aggregation
- Subscription-based architecture for easy integration
- Resilient connection handling with automatic reconnection
- Support for multiple data formats (JSON, SSE)

## How It Works

The client establishes a persistent connection to Pyth Network's streaming API and processes incoming price data. It organizes this data into time-based OHLC bars (candlesticks) that can be used for:

- Market data analysis
- Price charting
- Trading algorithms
- Historical data collection

## Features

- **Robust Connection Handling**: Implements automatic reconnection with exponential backoff
- **Efficient Stream Processing**: Handles chunked data and partial messages
- **Thread-safe Subscription Management**: Uses Rust's concurrency primitives for safe access
- **Multiple Message Format Support**: Processes both standard JSON and Server-Sent Events (SSE)
- **Daily Bar Aggregation**: Automatically creates and updates daily price bars

## Usage Example

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the subscription registry
    let subscriptions: Subscriptions = Arc::new(Mutex::new(HashMap::new()));
    
    // Start the streaming service in a separate task
    let subs_clone = subscriptions.clone();
    tokio::spawn(async move {
        let _ = start_streaming(subs_clone).await;
    });

    // Create a channel to receive bar updates
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Subscribe to BTC/USD with daily resolution
    let last_daily_bar = Bar {
        time: 1_717_000_000,
        open: 100.0,
        high: 100.0,
        low: 100.0,
        close: 100.0,
    };
    
    subscribe_on_stream(
        "BTC/USD".to_string(),
        "1D".to_string(),
        "sub1".to_string(),
        last_daily_bar,
        subscriptions.clone(),
        tx
    ).await;

    // Process incoming bar updates
    tokio::spawn(async move {
        while let Some(bar) = rx.recv().await {
            println!("Received bar: {:?}", bar);
            // Process the bar data as needed
        }
    });

    // Keep the application running until Ctrl+C
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

## Dependencies

- `tokio`: Asynchronous runtime
- `reqwest`: HTTP client with streaming support
- `serde`: JSON serialization/deserialization
- `futures-util`: Stream processing utilities
- `anyhow`: Error handling

## Getting Started

1. Add this to your Cargo.toml:
   ```toml
   [dependencies]
   pyth-benchmark-rs = { git = "https://github.com/yourusername/pyth-benchmark-rs" }
   ```

2. Use the client in your code:
   ```rust
   use pyth_benchmark_rs::{start_streaming, subscribe_on_stream, Subscriptions, Bar};
   ```

## License

[Specify your license here]

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
```

This README provides a comprehensive overview of your project, explaining that it's specifically built for reading OHLC data from the Pyth Network oracle. It includes information about the core functionality, usage examples, and setup instructions, making it easy for others to understand and use your repository.This README provides a comprehensive overview of your project, explaining that it's specifically built for reading OHLC data from the Pyth Network oracle. It includes information about the core functionality, usage examples, and setup instructions, making it easy for others to understand and use your repository.