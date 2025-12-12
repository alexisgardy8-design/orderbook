#[cfg(feature = "websocket")]
use tokio_tungstenite::{connect_async, tungstenite::Message};
#[cfg(feature = "websocket")]
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose};
use std::time::{SystemTime, UNIX_EPOCH};
use jsonwebtoken::{encode, EncodingKey, Header, Algorithm};
use rand::Rng;

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
    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error>> {
        dotenv::dotenv().ok();
        
        let api_key = std::env::var("API_KEY").ok();
        let secret_key = std::env::var("SECRET_KEY").ok();
        let passphrase = std::env::var("PASSPHRASE").ok();
        
        let (ws_stream, _) = connect_async(&self.url).await?;
        let (mut write, mut read) = ws_stream.split();
        
        let mut subscribe = SubscribeMessage {
            msg_type: "subscribe".to_string(),
            product_ids: self.products.clone(),
            channels: vec!["level2_batch".to_string(), "heartbeat".to_string()],
            signature: None,
            key: None,
            passphrase: None,
            timestamp: None,
        };
        
        // Coinbase Exchange WebSocket Authentication (HMAC-SHA256)
        // For authenticated level2 channel (optional - level2_batch works without auth)
        if let (Some(key), Some(secret), Some(pass)) = (&api_key, &secret_key, &passphrase) {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs()
                .to_string();
            
            // Create signature: timestamp + 'GET' + '/users/self/verify'
            let signature_path = "/users/self/verify";
            let message = format!("{}{}{}", timestamp, "GET", signature_path);
            
            // Decode base64 secret
            let secret_decoded = general_purpose::STANDARD.decode(secret)?;
            
            // Create HMAC-SHA256 signature
            let mut mac = Hmac::<Sha256>::new_from_slice(&secret_decoded)?;
            mac.update(message.as_bytes());
            let signature = general_purpose::STANDARD.encode(mac.finalize().into_bytes());
            
            subscribe.signature = Some(signature);
            subscribe.key = Some(key.clone());
            subscribe.passphrase = Some(pass.clone());
            subscribe.timestamp = Some(timestamp);
            
            println!("🔐 Authenticated with HMAC-SHA256 (Exchange API)");
            subscribe.channels = vec!["level2".to_string(), "heartbeat".to_string()];
        } else {
            println!("✅ Using public level2_batch channel (no auth needed)");
        }
        
        let subscribe_msg = serde_json::to_string(&subscribe)?;
        write.send(Message::Text(subscribe_msg)).await?;
        
        println!("✅ Connected to Coinbase WebSocket");
        println!("📡 Subscribing to: {:?} on channels: {:?}", self.products, subscribe.channels);
        
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Debug: print raw message type
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(msg_type) = value.get("type").and_then(|v| v.as_str()) {
                            match msg_type {
                                "subscriptions" => {
                                    println!("✅ Subscription confirmed!");
                                }
                                "snapshot" | "l2update" | "ticker" => {
                                    if let Ok(update) = serde_json::from_str::<CoinbaseL2Update>(&text) {
                                        self.process_update(update);
                                    }
                                }
                                "error" => {
                                    println!("❌ Error from Coinbase: {}", text);
                                }
                                _ => {
                                    println!("ℹ️  Message type: {}", msg_type);
                                }
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
    
    fn process_update(&self, update: CoinbaseL2Update) {
        match update.msg_type.as_str() {
            "snapshot" => {
                println!("\n📸 L2 Snapshot for {} ({} levels)", 
                    update.product_id, update.changes.len());
                
                let mut bids = 0;
                let mut asks = 0;
                for change in &update.changes {
                    if change.len() >= 3 {
                        if change[0] == "buy" { bids += 1; } else { asks += 1; }
                    }
                }
                println!("   Bids: {} | Asks: {}", bids, asks);
                
                if !update.changes.is_empty() {
                    let first = &update.changes[0];
                    if first.len() >= 3 {
                        println!("   Sample: {} @ {} (size: {})", first[0], first[1], first[2]);
                    }
                }
            }
            "l2update" => {
                println!("🔄 L2 Update {} - {} changes", update.product_id, update.changes.len());
                for change in &update.changes {
                    if change.len() >= 3 {
                        let side = if change[0] == "buy" { "🟢" } else { "🔴" };
                        println!("   {} {} @ {} (size: {})", side, change[0], change[1], change[2]);
                    }
                }
            }
            "ticker" => {
                if !update.best_bid.is_empty() && !update.best_ask.is_empty() {
                    let spread = if let (Ok(bid), Ok(ask)) = (update.best_bid.parse::<f64>(), update.best_ask.parse::<f64>()) {
                        ask - bid
                    } else {
                        0.0
                    };
                    println!("📊 {} - Bid: {} | Ask: {} | Spread: {:.4}", 
                        update.product_id, update.best_bid, update.best_ask, spread);
                }
            }
            _ => {}
        }
    }
}
