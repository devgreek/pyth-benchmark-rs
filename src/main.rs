use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use serde::Deserialize;
use tokio::sync::mpsc;
use anyhow::Result;
use std::time::{ Duration, UNIX_EPOCH, SystemTime };
use futures_util::StreamExt;
use bytes::Bytes;

const STREAMING_URL: &str = "https://benchmarks.pyth.network/v1/shims/tradingview/streaming";

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum StreamMessage {
    Data(StreamData),
    HeartbeatOrId(i64), // For heartbeats or numeric IDs
    Other(serde_json::Value), // For any other JSON format
}

#[derive(Debug, Deserialize, Clone)]
struct StreamData {
    id: String,
    p: f64,
    t: u64,
}

#[derive(Debug, Clone)]
struct Bar {
    time: u64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
}

#[derive(Clone)]
struct Handler {
    id: String,
    callback: mpsc::UnboundedSender<Bar>,
}

#[derive(Clone)]
struct Subscription {
    subscriber_uid: String,
    resolution: String,
    last_daily_bar: Bar,
    handlers: Vec<Handler>,
}

type Subscriptions = Arc<Mutex<HashMap<String, Subscription>>>;

fn get_next_daily_bar_time(bar_time: u64) -> u64 {
    // bar_time is in seconds since epoch
    let date = UNIX_EPOCH + Duration::from_secs(bar_time);
    let mut date_time = date + Duration::from_secs(60 * 60 * 24); // add 1 day
    date_time.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn handle_streaming_data(data: StreamData, subscriptions: &Subscriptions) {
    let mut map = subscriptions.lock().unwrap();
    let channel_string = &data.id;
    if let Some(sub_item) = map.get_mut(channel_string) {
        let trade_price = data.p;
        let trade_time = data.t * 1000; // milliseconds (not used, but follows JS logic)
        let last_bar = &sub_item.last_daily_bar;
        let next_daily_bar_time = get_next_daily_bar_time(last_bar.time);

        let bar = if data.t >= next_daily_bar_time {
            Bar {
                time: next_daily_bar_time,
                open: trade_price,
                high: trade_price,
                low: trade_price,
                close: trade_price,
            }
        } else {
            Bar {
                time: last_bar.time,
                open: last_bar.open,
                high: last_bar.high.max(trade_price),
                low: last_bar.low.min(trade_price),
                close: trade_price,
            }
        };

        // notify all subscribers
        for handler in &sub_item.handlers {
            let _ = handler.callback.send(bar.clone());
        }

        sub_item.last_daily_bar = bar;
    }
}

async fn start_streaming(subscriptions: Subscriptions) -> Result<()> {
    let client = reqwest::Client::new();
    let mut retries = 3;
    let mut delay = Duration::from_secs(3);
    loop {
        let resp = client.get(STREAMING_URL).send().await;
        if let Ok(response) = resp {
            // Reset retries when we get a successful connection
            retries = 3;
            delay = Duration::from_secs(3);

            println!("[stream] Connected to streaming endpoint");
            let mut stream = response.bytes_stream();

            while let Some(result) = stream.next().await {
                match result {
                    Ok(chunk) => {
                        let s = String::from_utf8_lossy(&chunk);
                        for line in s.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                // Debug log the incoming data
                                // println!("[stream] Received: {}", trimmed);

                                // Try parsing with our enum first
                                match serde_json::from_str::<StreamMessage>(trimmed) {
                                    Ok(StreamMessage::Data(json_data)) => {
                                        handle_streaming_data(json_data, &subscriptions);
                                    }
                                    Ok(StreamMessage::HeartbeatOrId(id)) => {
                                        println!("[stream] Received heartbeat/id: {}", id);
                                        // Handle heartbeat if needed
                                    }
                                    Ok(StreamMessage::Other(value)) => {
                                        println!(
                                            "[stream] Received other message format: {:?}",
                                            value
                                        );
                                        // Process other formats if needed
                                    }
                                    Err(e) => {
                                        // Be more specific about the error
                                        if trimmed == "{}" {
                                            // Empty object, likely a keepalive
                                            println!("[stream] Received empty object (keepalive)");
                                        } else if trimmed.starts_with("data: ") {
                                            // Server-sent events format
                                            let data_content = &trimmed[6..];
                                            println!("[stream] Detected SSE format, data: {}", data_content);

                                            // Try parsing the data part
                                            if !data_content.is_empty() {
                                                match
                                                    serde_json::from_str::<StreamMessage>(
                                                        data_content
                                                    )
                                                {
                                                    Ok(StreamMessage::Data(json_data)) => {
                                                        handle_streaming_data(
                                                            json_data,
                                                            &subscriptions
                                                        );
                                                    }
                                                    Ok(other) => {
                                                        println!(
                                                            "[stream] Parsed SSE data: {:?}",
                                                            other
                                                        );
                                                    }
                                                    Err(e2) => {
                                                        eprintln!(
                                                            "[stream] Error parsing SSE data: {:?}",
                                                            e2
                                                        );
                                                    }
                                                }
                                            }
                                        } else {
                                            eprintln!("[stream] Error parsing JSON: {:?}", e);
                                            eprintln!(
                                                "[stream] Raw data length: {}",
                                                trimmed.len()
                                            );
                                            // Print the first 100 chars to avoid flooding logs
                                            let preview = if trimmed.len() > 100 {
                                                &trimmed[..100]
                                            } else {
                                                trimmed
                                            };
                                            eprintln!("[stream] Raw data preview: {}", preview);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[stream] Error reading from stream: {:?}", e);
                        break; // Break to reconnect
                    }
                }
            }
            println!("[stream] Stream ended, will attempt to reconnect");
        } else {
            eprintln!("[stream] Error fetching from the streaming endpoint: {:?}", resp.err());
        }

        if retries > 0 {
            eprintln!("[stream] Attempting to reconnect in {:?}...", delay);
            tokio::time::sleep(delay).await;
            retries -= 1;
            // Implement exponential backoff
            delay *= 2;
        } else {
            eprintln!("[stream] Maximum reconnection attempts reached.");
            break;
        }
    }
    Ok(())
}

async fn subscribe_on_stream(
    symbol: String,
    resolution: String,
    subscriber_uid: String,
    last_daily_bar: Bar,
    subscriptions: Subscriptions,
    callback: mpsc::UnboundedSender<Bar>
) {
    let mut map = subscriptions.lock().unwrap();
    let handler = Handler { id: subscriber_uid.clone(), callback };
    let sub = Subscription {
        subscriber_uid,
        resolution,
        last_daily_bar,
        handlers: vec![handler],
    };
    map.insert(symbol.clone(), sub);
    println!("[subscribeBars]: Subscribe to streaming. Channel: {}", symbol);
    // You'd want to start streaming only once globally,
    // so call start_streaming() from your main or manager logic, not here.

}

fn unsubscribe_from_stream(subscriber_uid: &str, subscriptions: &Subscriptions) {
    let mut map = subscriptions.lock().unwrap();
    let keys: Vec<_> = map.keys().cloned().collect();
    for channel_string in keys {
        if let Some(sub_item) = map.get(&channel_string) {
            if sub_item.handlers.iter().any(|h| h.id == subscriber_uid) {
                println!("[unsubscribeBars]: Unsubscribe from streaming. Channel: {}", channel_string);
                map.remove(&channel_string);
                break;
            }
        }
    }
}

// -- Example usage --

#[tokio::main]
async fn main() -> Result<()> {
    let subscriptions: Subscriptions = Arc::new(Mutex::new(HashMap::new()));
    // You might spawn the streaming task once and keep subscriptions globally available
    let subs_clone = subscriptions.clone();
    tokio::spawn(async move {
        let _ = start_streaming(subs_clone).await;
    });

    // Simulate a subscription
    let (tx, mut rx) = mpsc::unbounded_channel();
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

    // Listen for bars (in your real app, this would be in a UI or logic handler)
    tokio::spawn(async move {
        while let Some(bar) = rx.recv().await {
            println!("Received bar: {:?}", bar);
        }
    });

    // Keep the main alive (for demonstration)
    tokio::signal::ctrl_c().await?;
    Ok(())
}
