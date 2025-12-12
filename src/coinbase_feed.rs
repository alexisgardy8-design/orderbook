#[cfg(feature = "websocket")]
use tokio_tungstenite::{connect_async, tungstenite::Message};
#[cfg(feature = "websocket")]
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct CoinbaseL2Update {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub product_id: String,
    #[serde(default)]
    pub changes: Vec<Vec<String>>,
    pub time: String,
    #[serde(default)]
    pub best_bid: String,
    #[serde(default)]
    pub best_ask: String,
    #[serde(default)]
    pub price: String,
}

#[derive(Debug, Serialize)]
struct SubscribeMessage {
    #[serde(rename = "type")]
    msg_type: String,
    product_ids: Vec<String>,
    channels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    passphrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
}

pub struct CoinbaseFeed {
    pub url: String,
    pub products: Vec<String>,
}

impl CoinbaseFeed {
    pub fn new(products: Vec<String>) -> Self {
        Self {
            url: "wss://ws-feed.exchange.coinbase.com".to_string(),
            products,
        }
    }
    
    #[cfg(feature = "websocket")]
    pub async fn connect_with_arbitrage(&self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::interfaces::OrderBook;
        use crate::orderbook::OrderBookImpl;
        use crate::triangular_arbitrage::TriangularArbitrageDetector;
        use std::time::Instant;
        
        // Initialize orderbooks for the three pairs
        let mut orderbooks = HashMap::new();
        orderbooks.insert("ATOM-USD".to_string(), Arc::new(Mutex::new(OrderBookImpl::new())));
        orderbooks.insert("ATOM-BTC".to_string(), Arc::new(Mutex::new(OrderBookImpl::new())));
        orderbooks.insert("BTC-USD".to_string(), Arc::new(Mutex::new(OrderBookImpl::new())));
        
        // Create arbitrage detector
        let detector = Arc::new(Mutex::new(
            TriangularArbitrageDetector::new(0.2) // 0.2% minimum profit
        ));
        
        let opportunities_found = Arc::new(Mutex::new(0u64));
        let updates_processed = Arc::new(Mutex::new(0u64));
        let total_processing_time = Arc::new(Mutex::new(0u128));
        
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        // Subscribe to level2_batch (public, no auth)
        let subscribe = SubscribeMessage {
            msg_type: "subscribe".to_string(),
            product_ids: self.products.clone(),
            channels: vec!["level2_batch".to_string(), "heartbeat".to_string()],
            signature: None,
            key: None,
            passphrase: None,
            timestamp: None,
        };
        
        let subscribe_msg = serde_json::to_string(&subscribe)?;
        write.send(Message::Text(subscribe_msg)).await?;
        
        println!("✅ Connected to Coinbase WebSocket");
        println!("📡 Subscribing to: {:?} on level2_batch", self.products);
        println!("\n🚀 Live Arbitrage Detection Started!");
        println!("   Fee: 0.1% | Min Profit: 0.2%\n");
        
        let mut update_count = 0u64;
        let start_time = Instant::now();
        
        // Process messages
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = value.get("type").and_then(|v| v.as_str()) {
                            match msg_type {
                                "subscriptions" => {
                                    println!("✅ Subscription confirmed!");
                                }
                                "snapshot" | "l2update" => {
                                    if let Ok(update) = serde_json::from_str::<CoinbaseL2Update>(&text) {
                                        let process_start = Instant::now();
                                        
                                        self.process_arbitrage_update(
                                            update,
                                            &orderbooks,
                                            &detector,
                                            &opportunities_found,
                                            &updates_processed,
                                            &total_processing_time,
                                        );
                                        
                                        let process_time = process_start.elapsed().as_nanos();
                                        *total_processing_time.lock().unwrap() += process_time;
                                        
                                        update_count += 1;
                                        if update_count % 100 == 0 {
                                            let elapsed = start_time.elapsed().as_secs_f64();
                                            let updates = *updates_processed.lock().unwrap();
                                            let opps = *opportunities_found.lock().unwrap();
                                            let avg_time = if updates > 0 {
                                                *total_processing_time.lock().unwrap() / updates as u128
                                            } else {
                                                0
                                            };
                                            
                                            println!("\n📊 Performance Stats:");
                                            println!("   Updates: {} | Opps: {} | Rate: {:.0} updates/s", 
                                                updates, opps, updates as f64 / elapsed);
                                            println!("   Avg Processing: {} ns | Target: <1ns", avg_time);
                                        }
                                    }
                                }
                                "error" => {
                                    println!("❌ Error from Coinbase: {}", text);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Err(e) => {
                    eprintln!("❌ WebSocket error: {:?}", e);
                    break;
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    fn process_arbitrage_update(
        &self,
        update: CoinbaseL2Update,
        orderbooks: &HashMap<String, Arc<Mutex<crate::orderbook::OrderBookImpl>>>,
        detector: &Arc<Mutex<crate::triangular_arbitrage::TriangularArbitrageDetector>>,
        opportunities_found: &Arc<Mutex<u64>>,
        updates_processed: &Arc<Mutex<u64>>,
        _total_time: &Arc<Mutex<u128>>,
    ) {
        use crate::interfaces::{Update, Side, OrderBook};
        
        if let Some(ob) = orderbooks.get(&update.product_id) {
            let mut ob_lock = ob.lock().unwrap();
            
            // Apply updates to orderbook
            for change in &update.changes {
                if change.len() >= 3 {
                    let side = if change[0] == "buy" { Side::Bid } else { Side::Ask };
                    let price = change[1].parse::<f64>().unwrap_or(0.0);
                    let quantity = change[2].parse::<f64>().unwrap_or(0.0);
                    
                    // Convert price to integer with 4 decimal places precision
                    let price_int = (price * 10000.0) as i64;
                    
                    if price_int >= 0 && price_int < 200_000 {
                        let update = Update::Set {
                            side,
                            price: price_int,
                            quantity: (quantity * 1_000_000.0) as u64, // Convert to micros
                        };
                        
                        ob_lock.apply_update(update);
                    }
                }
            }
            
            drop(ob_lock);
            
            let update_count = {
                let mut count = updates_processed.lock().unwrap();
                *count += 1;
                *count
            };
            
            // Check for arbitrage opportunities every 10 updates to reduce lock contention
            if update_count % 10 == 0 {
                let ob1 = orderbooks.get("ATOM-USD").unwrap().lock().unwrap();
                let ob2 = orderbooks.get("ATOM-BTC").unwrap().lock().unwrap();
                let ob3 = orderbooks.get("BTC-USD").unwrap().lock().unwrap();
                
                // Debug: print prices every 100 updates
                if update_count % 100 == 0 {
                    println!("\n🔍 Current Orderbook Prices:");
                    println!("   ATOM-USD: Bid={:?} Ask={:?}", 
                        ob1.get_best_bid().map(|p| p as f64 / 10000.0),
                        ob1.get_best_ask().map(|p| p as f64 / 10000.0));
                    println!("   ATOM-BTC: Bid={:?} Ask={:?}",
                        ob2.get_best_bid().map(|p| p as f64 / 10000.0),
                        ob2.get_best_ask().map(|p| p as f64 / 10000.0));
                    println!("   BTC-USD:  Bid={:?} Ask={:?}",
                        ob3.get_best_bid().map(|p| p as f64 / 10000.0),
                        ob3.get_best_ask().map(|p| p as f64 / 10000.0));
                }
                
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                let mut det = detector.lock().unwrap();
                let opportunities = det.detect_opportunities_with_refs(&*ob1, &*ob2, &*ob3, timestamp, 1000.0);
                
                drop(det);
                drop(ob1);
                drop(ob2);
                drop(ob3);
                
                if !opportunities.is_empty() {
                    let mut opps = opportunities_found.lock().unwrap();
                    *opps += opportunities.len() as u64;
                    
                    for opp in opportunities {
                        println!("\n🎯 ARBITRAGE OPPORTUNITY DETECTED!");
                        println!("   Path: {:?}", opp.path);
                        println!("   Profit: ${:.2} ({:.2}%)", opp.net_profit, opp.profit_percentage * 100.0);
                        println!("   Input: ${:.2} | Output: ${:.2}", opp.input_amount, opp.expected_output);
                    }
                }
            }
        }
    }
}
