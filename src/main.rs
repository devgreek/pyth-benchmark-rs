//! Example usage of the pyth-benchmark-rs crate
//!
//! This example demonstrates how to:
//! - Create a DataFeed instance
//! - Fetch historical bar data
//! - Start streaming real-time data
//! - Subscribe to symbol updates

use pyth_benchmark_rs::{ DataFeed, SymbolInfo, PeriodParams, start_streaming };
use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 Starting Pyth Benchmark Rust Client Example");

    // Initialize the subscription registry
    let subscriptions = Arc::new(Mutex::new(HashMap::new()));

    // Create a DataFeed instance
    let datafeed = DataFeed::new(subscriptions.clone());

    // Initialize the data feed
    datafeed.on_ready(|config| {
        println!("📊 DataFeed ready with config: {:?}", config);
    }).await;

    // Example 1: Get historical data for ETH/USD
    println!("\n📈 Fetching historical data for ETH/USD...");
    let symbol_info = SymbolInfo::new("Crypto.ETH/USD");
    let period_params = PeriodParams::last_days(20, true); // Last 30 days

    datafeed.get_bars(
        &symbol_info,
        "60", // Daily bars
        &period_params,
        |bars, no_data| {
            if no_data {
                println!("❌ No historical data available");
            } else {
                println!("✅ Received {} historical bars", bars.len());

                // Show first few bars
                for (i, bar) in bars.iter().enumerate().take(100) {
                    println!(
                        "   Bar {}: time={}, O={:.2}, H={:.2}, L={:.2}, C={:.2}",
                        i + 1,
                        bar.time,
                        bar.open,
                        bar.high,
                        bar.low,
                        bar.close
                    );
                }

                if bars.len() > 10 {
                    println!("   ... and {:?} more bars", bars);
                }
            }
        },
        |err| {
            eprintln!("❌ Error fetching historical data: {}", err);
        }
    ).await;

    // Example 2: Start streaming real-time data
    // println!("\n🔄 Starting real-time data streaming...");
    // let subs_clone = subscriptions.clone();
    // tokio::spawn(async move {
    //     if let Err(e) = start_streaming(subs_clone).await {
    //         eprintln!("❌ Streaming error: {}", e);
    //     }
    // });

    // Example 3: Subscribe to real-time bar updates
    // println!("📡 Subscribing to BTC/USD real-time updates...");
    // let (tx, mut rx) = mpsc::unbounded_channel();

    // let btc_symbol = SymbolInfo::new("Crypto.BTC/USD");
    // datafeed.subscribe_bars(
    //     btc_symbol,
    //     "1D".to_string(),
    //     tx,
    //     "example_subscriber".to_string(),
    //     None
    // ).await;

    // Example 4: Listen for real-time updates
    // println!("👂 Listening for real-time bar updates (press Ctrl+C to stop)...\n");

    // let update_task = tokio::spawn(async move {
    //     let mut update_count = 0;
    //     while let Some(bar) = rx.recv().await {
    //         update_count += 1;
    //         println!(
    //             "🔔 Update #{}: BTC/USD - O:{:.2} H:{:.2} L:{:.2} C:{:.2} T:{}",
    //             update_count,
    //             bar.open,
    //             bar.high,
    //             bar.low,
    //             bar.close,
    //             bar.time
    //         );

    //         // Limit output for demo purposes
    //         if update_count >= 10 {
    //             println!("📊 Received {} updates, stopping demo...", update_count);
    //             break;
    //         }
    //     }
    // });

    // // Wait for either the update task to complete or Ctrl+C
    // tokio::select! {
    //     _ = update_task => {
    //         println!("✅ Demo completed successfully!");
    //     }
    //     _ = tokio::signal::ctrl_c() => {
    //         println!("\n🛑 Received interrupt signal, shutting down...");
    //     }
    // }

    // // Clean up subscription
    // datafeed.unsubscribe_bars("example_subscriber").await;
    // println!("🧹 Cleaned up subscriptions");

    Ok(())
}
