use crate::types::{
    Bar, StreamData, StreamMessage, Subscription, Subscriptions
};
use crate::STREAMING_URL;
use anyhow::Result;
use futures_util::StreamExt;
use std::time::{Duration, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Start streaming price data from Pyth Network
/// 
/// This function establishes a persistent connection to the Pyth streaming endpoint
/// and processes incoming price data, updating subscribed handlers with new bars.
/// 
/// # Arguments
/// * `subscriptions` - Thread-safe collection of active subscriptions
/// 
/// # Returns
/// * `Result<()>` - Ok if streaming completed successfully, Err on fatal error
pub async fn start_streaming(subscriptions: Subscriptions) -> Result<()> {
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
                                process_stream_line(trimmed, &subscriptions);
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
            eprintln!("[stream] Maximum reconnection attempts reached. Resetting retry counter");
            // Reset retries with a longer delay instead of giving up
            retries = 3;
            delay = Duration::from_secs(30);
        }
    }
}

/// Process a single line from the stream
fn process_stream_line(line: &str, subscriptions: &Subscriptions) {
    match serde_json::from_str::<StreamMessage>(line) {
        Ok(StreamMessage::Data(json_data)) => {
            handle_streaming_data(json_data, subscriptions);
        }
        Ok(StreamMessage::HeartbeatOrId(id)) => {
            // Only log occasional heartbeats to reduce noise
            if id % 100 == 0 {
                println!("[stream] Received heartbeat/id: {}", id);
            }
        }
        Ok(StreamMessage::Other(value)) => {
            println!("[stream] Received other message format: {:?}", value);
        }
        Err(e) => {
            // Handle special cases
            if line == "{}" {
                // Empty object, likely a keepalive
                return;
            } else if line.starts_with("data: ") {
                // Server-sent events format
                let data_content = &line[6..];
                if !data_content.is_empty() {
                    process_stream_line(data_content, subscriptions);
                }
            } else {
                eprintln!("[stream] Error parsing JSON: {:?}", e);
                eprintln!("[stream] Raw data: {}", line);
            }
        }
    }
}

/// Subscribe to streaming data for a specific symbol
/// 
/// # Arguments
/// * `symbol` - Symbol identifier
/// * `resolution` - Time resolution
/// * `subscriber_uid` - Unique subscriber identifier
/// * `last_daily_bar` - Last known daily bar for this symbol
/// * `subscriptions` - Subscriptions registry
/// * `callback` - Channel for sending bar updates
pub async fn subscribe_on_stream(
    symbol: String,
    resolution: String,
    subscriber_uid: String,
    last_daily_bar: Bar,
    subscriptions: Subscriptions,
    callback: mpsc::UnboundedSender<Bar>,
) {
    if let Ok(mut map) = subscriptions.lock() {
        let handler = crate::types::Handler {
            id: subscriber_uid.clone(),
            callback,
        };
        
        let sub = Subscription {
            subscriber_uid,
            resolution,
            last_daily_bar,
            handlers: vec![handler],
        };
        
        map.insert(symbol.clone(), sub);
        println!("[subscribeBars]: Subscribe to streaming. Channel: {}", symbol);
    }
}

/// Unsubscribe from streaming data
/// 
/// # Arguments
/// * `subscriber_uid` - Unique identifier of the subscription to remove
/// * `subscriptions` - Subscriptions registry
pub fn unsubscribe_from_stream(subscriber_uid: &str, subscriptions: &Subscriptions) {
    if let Ok(mut map) = subscriptions.lock() {
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
}

/// Handle incoming streaming data and update bars
fn handle_streaming_data(data: StreamData, subscriptions: &Subscriptions) {
    if let Ok(mut map) = subscriptions.lock() {
        let channel_string = &data.id;
        
        if let Some(sub_item) = map.get_mut(channel_string) {
            let trade_price = data.p;
            let last_bar = &sub_item.last_daily_bar;
            let next_daily_bar_time = get_next_daily_bar_time(last_bar.time);

            let bar = if data.t >= next_daily_bar_time {
                // Start a new daily bar
                Bar {
                    time: next_daily_bar_time,
                    open: trade_price,
                    high: trade_price,
                    low: trade_price,
                    close: trade_price,
                }
            } else {
                // Update current daily bar
                Bar {
                    time: last_bar.time,
                    open: last_bar.open,
                    high: last_bar.high.max(trade_price),
                    low: last_bar.low.min(trade_price),
                    close: trade_price,
                }
            };

            // Notify all subscribers
            for handler in &sub_item.handlers {
                if let Err(e) = handler.callback.send(bar.clone()) {
                    eprintln!("[stream] Failed to send bar update: {:?}", e);
                }
            }

            sub_item.last_daily_bar = bar;
        }
    }
}

/// Calculate the timestamp for the next daily bar
fn get_next_daily_bar_time(bar_time: u64) -> u64 {
    // bar_time is in milliseconds since epoch
    let seconds = bar_time / 1000;
    let date = UNIX_EPOCH + Duration::from_secs(seconds);
    let next_date = date + Duration::from_secs(24 * 60 * 60); // add 1 day
    
    next_date
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() * 1000 // Convert back to milliseconds
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_get_next_daily_bar_time() {
        let timestamp = 1_717_000_000_000; // Some timestamp in milliseconds
        let next = get_next_daily_bar_time(timestamp);
        
        // Should be exactly 24 hours later
        assert_eq!(next - timestamp, 24 * 60 * 60 * 1000);
    }

    #[test]
    fn test_process_stream_line_empty() {
        let subscriptions = Arc::new(Mutex::new(HashMap::new()));
        
        // Should not panic on empty JSON object
        process_stream_line("{}", &subscriptions);
    }
}
