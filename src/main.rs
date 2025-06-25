use std::collections::HashMap;
use std::sync::{ Arc, Mutex };
use serde::{ Deserialize, Serialize };
use reqwest::Client;
use tokio::sync::mpsc;
use anyhow::Result;
use std::time::{ Duration, UNIX_EPOCH, SystemTime };
use futures_util::StreamExt;
use bytes::Bytes;

const STREAMING_URL: &str = "https://benchmarks.pyth.network/v1/shims/tradingview/streaming";
const API_ENDPOINT: &str = "https://benchmarks.pyth.network/v1/shims/tradingview";

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

#[derive(Clone)]
pub struct Handler {
    pub id: String,
    pub callback: mpsc::UnboundedSender<Bar>,
}

#[derive(Clone)]
pub struct Subscription {
    pub subscriber_uid: String,
    pub resolution: String,
    pub last_daily_bar: Bar,
    pub handlers: Vec<Handler>,
}

type Subscriptions = Arc<Mutex<HashMap<String, Subscription>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub time: u64,
    pub low: f64,
    pub high: f64,
    pub open: f64,
    pub close: f64,
}

type LastBarsCache = Arc<Mutex<HashMap<String, Bar>>>;

pub struct DataFeed {
    last_bars_cache: LastBarsCache,
    client: Client,
    subscriptions: Subscriptions,
}

impl DataFeed {
    pub fn new(subscriptions: Subscriptions) -> Self {
        DataFeed {
            last_bars_cache: Arc::new(Mutex::new(HashMap::new())),
            client: Client::new(),
            subscriptions,
        }
    }

    pub async fn on_ready<F>(&self, mut callback: F)
        where F: FnMut(serde_json::Value) + Send + 'static
    {
        println!("[onReady]: Method call");
        let url = format!("{}/config", API_ENDPOINT);
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(config) = resp.json::<serde_json::Value>().await {
                println!("{:?}", config);
                // Call the callback with the config data
                println!("[onReady]: Config data received");
                // Simulate setTimeout with tokio::spawn
                tokio::spawn(async move {
                    callback(config);
                });
            }
        }
    }

    pub async fn search_symbols<F>(
        &self,
        user_input: &str,
        _exchange: &str,
        _symbol_type: &str,
        mut on_result_ready_callback: F
    )
        where F: FnMut(serde_json::Value) + Send + 'static
    {
        println!("[searchSymbols]: Method call");
        let url = format!("{}/search?query={}", API_ENDPOINT, user_input);
        if let Ok(resp) = self.client.get(&url).send().await {
            if let Ok(data) = resp.json().await {
                on_result_ready_callback(data);
            }
        }
    }

    pub async fn resolve_symbol<F1, F2>(
        &self,
        symbol_name: &str,
        mut on_symbol_resolved_callback: F1,
        mut on_resolve_error_callback: F2
    )
        where F1: FnMut(serde_json::Value) + Send + 'static, F2: FnMut(&str) + Send + 'static
    {
        println!("[resolveSymbol]: Method call {}", symbol_name);
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
        }
    }

    pub async fn get_bars<F1, F2>(
        &self,
        symbol_info: &SymbolInfo,
        resolution: &str,
        period_params: &PeriodParams,
        mut on_history_callback: F1,
        mut on_error_callback: F2
    )
        where F1: FnMut(Vec<Bar>, bool) + Send + 'static, F2: FnMut(anyhow::Error) + Send + 'static
    {
        let SymbolInfo { ticker } = symbol_info;
        let (from, to, first_data_request) = (
            period_params.from,
            period_params.to,
            period_params.first_data_request,
        );

        println!("[getBars]: Method call {} {} {} {}", ticker, resolution, from, to);

        let max_range = 365 * 24 * 60 * 60;
        let mut promises = vec![];
        let mut current_from = from;
        let mut current_to;

        while current_from < to {
            current_to = std::cmp::min(to, current_from + max_range);
            let url: String = format!(
                "{}/history?symbol={}&resolution={}&from={}&to={}",
                API_ENDPOINT,
                ticker,
                resolution,
                current_from,
                current_to
                
            );
            println!("{}", url);
            promises.push(self.client.get(&url).send());
            current_from = current_to;
        }

        let mut bars: Vec<Bar> = vec![];

        let results = futures_util::future::join_all(promises).await;
        for resp in results {
            if let Ok(r) = resp {
                if let Ok(text) = r.text().await {
                    if let Ok(data) = serde_json::from_str::<HistoryResponse>(&text) {
                        if !data.t.is_empty() {
                            for (i, time) in data.t.iter().enumerate() {
                                bars.push(Bar {
                                    time: time * 1000,
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

        println!("{:?}", bars);
        
        if first_data_request && !bars.is_empty() {
            self.last_bars_cache
                .lock()
                .unwrap()
                .insert(ticker.clone(), bars.last().unwrap().clone());
        }

        on_history_callback(bars.clone(), bars.is_empty());
    }

    pub async fn subscribe_bars(
        &self,
        symbol_info: SymbolInfo,
        resolution: String,
        on_realtime_callback: mpsc::UnboundedSender<Bar>,
        subscriber_uid: String,
        on_reset_cache_needed_callback: Option<fn()>
    ) {
        println!("[subscribeBars]: Method call with subscriberUID: {}", subscriber_uid);
        let last_bar = self.last_bars_cache.lock().unwrap().get(&symbol_info.ticker).cloned();

        println!("{:?}", last_bar);

        if let Some(bar) = last_bar {
            subscribe_on_stream(
                symbol_info.ticker.clone(),
                resolution,
                subscriber_uid,
                bar,
                self.subscriptions.clone(),
                on_realtime_callback
            ).await;
        } else {
            println!("[subscribeBars]: No last bar available for {}", symbol_info.ticker);
        }
    }
    pub async fn unsubscribe_bars(&self, subscriber_uid: &str) {
        println!("[unsubscribeBars]: Method call with subscriberUID: {}", subscriber_uid);
        unsubscribe_from_stream(subscriber_uid, &self.subscriptions);
    }
}

// --- Helper structs ---

#[derive(Clone, Debug)]
pub struct SymbolInfo {
    pub ticker: String,
    // add other fields as needed
}

#[derive(Clone, Debug)]
pub struct PeriodParams {
    pub from: u64,
    pub to: u64,
    pub first_data_request: bool,
    // add other fields as needed
}

#[derive(Deserialize, Debug)]
struct HistoryResponse {
    t: Vec<u64>,
    l: Vec<f64>,
    h: Vec<f64>,
    o: Vec<f64>,
    c: Vec<f64>,
}

// --- Usage Example ---
// let datafeed = DataFeed::new();
// datafeed.on_ready(|config| println!("Config: {:?}", config)).await;

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
    // let subs_clone = subscriptions.clone();
    // tokio::spawn(async move {
    //     let _ = start_streaming(subs_clone).await;
    // });

    // // Simulate a subscription
    // let (tx, mut rx) = mpsc::unbounded_channel();
    // let last_daily_bar = Bar {
    //     time: 1_717_000_000,
    //     open: 100.0,
    //     high: 100.0,
    //     low: 100.0,
    //     close: 100.0,
    // };
    // subscribe_on_stream(
    //     "BTC/USD".to_string(),
    //     "1D".to_string(),
    //     "sub1".to_string(),
    //     last_daily_bar,
    //     subscriptions.clone(),
    //     tx
    // ).await;

    // // Listen for bars (in your real app, this would be in a UI or logic handler)
    // tokio::spawn(async move {
    //     while let Some(bar) = rx.recv().await {
    //         println!("Received bar: {:?}", bar);
    //     }
    // });

    // // Keep the main alive (for demonstration)
    // tokio::signal::ctrl_c().await?;

    // Create a DataFeed instance
    // let datafeed = DataFeed::new(subscriptions.clone());

    // // Create symbol info for BTC/USD
    // let symbol_info = SymbolInfo {
    //     ticker: "BTC/USD".to_string(),
    // };

    // // Create channel for realtime bar updates
    // let (tx, mut rx) = mpsc::unbounded_channel();

    // // Subscribe to bars using the DataFeed API
    // datafeed.subscribe_bars(
    //     symbol_info,
    //     "1D".to_string(),
    //     tx,
    //     "datafeed_sub1".to_string(),
    //     None
    // ).await;

    // // Process received bars
    // tokio::spawn(async move {
    //     while let Some(bar) = rx.recv().await {
    //         println!("Received bar from DataFeed subscription: {:?}", bar);
    //     }
    // });

    // let subscriptions: Subscriptions = Arc::new(Mutex::new(HashMap::new()));
    // // You might spawn the streaming task once and keep subscriptions globally available
    // let subs_clone = subscriptions.clone();
    // tokio::spawn(async move {
    //     let _ = start_streaming(subs_clone).await;
    // });

    // // Create a DataFeed instance
    let datafeed = DataFeed::new(subscriptions.clone());

    // // Call the on_ready method
    // datafeed.on_ready(|config| {
    //     println!("DataFeed ready with config: {:?}", config);
    // }).await;

    // // Simulate a subscription
    // let (tx, mut rx) = mpsc::unbounded_channel();
    // let last_daily_bar = Bar {
    //     time: 1_717_000_000,
    //     open: 100.0,
    //     high: 100.0,
    //     low: 100.0,
    //     close: 100.0,
    // };
    // subscribe_on_stream(
    //     "BTC/USD".to_string(),
    //     "1D".to_string(),
    //     "sub1".to_string(),
    //     last_daily_bar,
    //     subscriptions.clone(),
    //     tx
    // ).await;

    // Create symbol info for BTC/USD
    let symbol_info = SymbolInfo {
        ticker: "Crypto.ETH/USD".to_string(),
    };

    // Define resolution
    let resolution = "1D";

    // Define period parameters (from 30 days ago to now)
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
     let period_params = PeriodParams {
        from: now - 20 * 24 * 60 * 60, // 30 days ago
        to: now,
        first_data_request: true,
    };

    println!("{:?}", now);
    println!("{:?}",  now - 1 * 24 * 60 * 60);

    // Call get_bars with appropriate callbacks
    datafeed.get_bars(
        &symbol_info,
        resolution,
        &period_params,
        |bars, no_data| {
            println!("Received {} bars. No data: {}", bars.len(), no_data);
            // println!("", bar);
            for (i, bar) in bars.iter().enumerate().take(5) {
                println!("Bar {}: time={}, open={}, high={}, low={}, close={}", 
                    i, bar.time, bar.open, bar.high, bar.low, bar.close);
            }
            if bars.len() > 5 {
                println!("... and {} more bars", bars.len() - 5);
            }
        },
        |err| {
            eprintln!("Error fetching bars: {}", err);
        }
    ).await;

    Ok(())
}
