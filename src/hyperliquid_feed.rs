// 🚀 Hyperliquid WebSocket Feed - Live Trading Bot
// Connexion au DEX Hyperliquid pour récupérer les données SOL-PERP en temps réel
// Calcul des indicateurs (ADX, SuperTrend, Bollinger) et génération de signaux

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;

const HYPERLIQUID_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const COIN: &str = "SOL";
const CANDLE_INTERVAL: &str = "1m"; // 1 minute pour tests, 1h pour production
const CANDLE_BUFFER_SIZE: usize = 100; // Garder 100 bougies pour les indicateurs

/// Bougie OHLCV de Hyperliquid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperCandle {
    pub t: u64,      // open timestamp (millis)
    #[serde(rename = "T")]
    pub close_t: u64, // close timestamp (millis)
    pub s: String,   // coin symbol
    pub i: String,   // interval
    pub o: f64,      // open price
    pub c: f64,      // close price
    pub h: f64,      // high price
    pub l: f64,      // low price
    pub v: f64,      // volume
    pub n: u64,      // number of trades
}

/// Message WebSocket de Hyperliquid
#[derive(Debug, Deserialize)]
struct WebSocketMessage {
    channel: String,
    data: serde_json::Value,
}

/// Message de confirmation d'abonnement
#[derive(Debug, Deserialize)]
struct SubscriptionResponse {
    method: String,
    subscription: serde_json::Value,
}

/// Client WebSocket pour Hyperliquid
pub struct HyperliquidFeed {
    coin: String,
    interval: String,
    candle_buffer: VecDeque<HyperCandle>,
    strategy: crate::adaptive_strategy::AdaptiveStrategy,
}

impl HyperliquidFeed {
    pub fn new(coin: String, interval: String) -> Self {
        use crate::adaptive_strategy::AdaptiveConfig;
        
        Self {
            coin,
            interval,
            candle_buffer: VecDeque::with_capacity(CANDLE_BUFFER_SIZE),
            strategy: crate::adaptive_strategy::AdaptiveStrategy::new(AdaptiveConfig {
                adx_threshold: 20.0,
                ..Default::default()
            }),
        }
    }

    /// Connexion au WebSocket et traitement des messages
    pub async fn connect_and_trade(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║  🚀 HYPERLIQUID LIVE TRADING BOT - ADAPTIVE STRATEGY          ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        
        println!("⚙️  Configuration:");
        println!("   DEX:          Hyperliquid");
        println!("   Pair:         {}-PERP", self.coin);
        println!("   Timeframe:    {}", self.interval);
        println!("   Strategy:     Adaptive Bidirectional (Long + Short)");
        println!("   ADX Threshold: 20.0");
        println!("   Mode:         DRY RUN (signaux uniquement)\n");

        println!("🌐 Connecting to Hyperliquid WebSocket...");
        let (ws_stream, _) = connect_async(HYPERLIQUID_WS_URL).await?;
        println!("✅ Connected to {}\n", HYPERLIQUID_WS_URL);

        let (mut write, mut read) = ws_stream.split();

        // S'abonner aux bougies 1h pour SOL
        let subscribe_msg = json!({
            "method": "subscribe",
            "subscription": {
                "type": "candle",
                "coin": self.coin,
                "interval": self.interval
            }
        });

        println!("📡 Subscribing to {} candles ({})...", self.coin, self.interval);
        write.send(Message::Text(subscribe_msg.to_string())).await?;

        // Traiter les messages entrants
        let mut message_count = 0;
        let mut candle_count = 0;

        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    message_count += 1;

                    // Parser le message
                    if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                        match ws_msg.channel.as_str() {
                            "subscriptionResponse" => {
                                if let Ok(resp) = serde_json::from_value::<SubscriptionResponse>(ws_msg.data) {
                                    println!("✅ Subscription confirmed: {:?}\n", resp.subscription);
                                    println!("🔄 Waiting for candle data...\n");
                                }
                            }
                            "candle" => {
                                // Parser les bougies
                                if let Ok(candles) = serde_json::from_value::<Vec<HyperCandle>>(ws_msg.data) {
                                    for candle in candles {
                                        candle_count += 1;
                                        self.process_candle(candle, candle_count);
                                    }
                                }
                            }
                            _ => {
                                // Ignorer les autres channels
                            }
                        }
                    }

                    // Afficher un heartbeat toutes les 50 messages
                    if message_count % 50 == 0 {
                        println!("💓 Heartbeat - Messages: {}, Candles: {}, Buffer: {}", 
                            message_count, candle_count, self.candle_buffer.len());
                    }
                }
                Ok(Message::Ping(data)) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Message::Close(_)) => {
                    println!("\n⚠️  Connection closed by server");
                    break;
                }
                Err(e) => {
                    eprintln!("❌ WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Traite une bougie reçue et génère des signaux
    fn process_candle(&mut self, candle: HyperCandle, count: usize) {
        // Ajouter au buffer
        self.candle_buffer.push_back(candle.clone());
        if self.candle_buffer.len() > CANDLE_BUFFER_SIZE {
            self.candle_buffer.pop_front();
        }

        // Afficher la bougie
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🕯️  CANDLE #{} - {} {}", count, candle.s, self.interval);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("   Open:   ${:.2}", candle.o);
        println!("   High:   ${:.2}", candle.h);
        println!("   Low:    ${:.2}", candle.l);
        println!("   Close:  ${:.2}", candle.c);
        println!("   Volume: {:.2}", candle.v);
        println!("   Trades: {}", candle.n);
        
        let change_pct = ((candle.c - candle.o) / candle.o) * 100.0;
        let color = if change_pct > 0.0 { "🟢" } else { "🔴" };
        println!("   Change: {} {:+.2}%", color, change_pct);

        // Calculer les indicateurs si on a assez de données
        if self.candle_buffer.len() >= 50 { // Minimum pour ADX (14) + SuperTrend (10) + Bollinger (20)
            let signal = self.strategy.update(candle.h, candle.l, candle.c);
            
            println!("\n📊 STRATEGY ANALYSIS:");
            self.display_indicators();
            self.display_signal(signal, candle.c);
        } else {
            println!("\n⏳ Warming up indicators... ({}/50 candles)", self.candle_buffer.len());
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }

    /// Affiche les indicateurs calculés
    fn display_indicators(&self) {
        use crate::adaptive_strategy::MarketRegime;
        
        let regime = self.strategy.get_current_regime();
        let adx = self.strategy.get_adx_value();
        let position = self.strategy.get_position_type();

        println!("   ADX Value:      {:.2}", adx);
        println!("   Market Regime:  {:?}", regime);
        println!("   Position Type:  {:?}", position);
        
        match regime {
            MarketRegime::Ranging => {
                println!("   Mode:           🎯 RANGE (Bollinger Mean Reversion)");
            }
            MarketRegime::Trending => {
                println!("   Mode:           🚀 TREND (SuperTrend Bidirectional)");
            }
        }
    }

    /// Affiche le signal de trading
    fn display_signal(&self, signal: crate::adaptive_strategy::Signal, current_price: f64) {
        use crate::adaptive_strategy::Signal;

        println!("\n🎯 TRADING SIGNAL:");
        
        match signal {
            Signal::BuyRange => {
                println!("   🟢 BUY RANGE (Long)");
                println!("   Entry: ${:.2}", current_price);
                println!("   Reason: Price touched lower Bollinger band (oversold)");
                println!("   Target: Middle band (mean reversion)");
                println!("   Strategy: Long only in range mode");
            }
            Signal::SellRange => {
                println!("   🔴 SELL RANGE (Close Long)");
                println!("   Exit: ${:.2}", current_price);
                println!("   Reason: Price returned to mean");
            }
            Signal::BuyTrend => {
                println!("   🚀 BUY TREND (Long)");
                println!("   Entry: ${:.2}", current_price);
                println!("   Reason: Strong uptrend detected (SuperTrend + ADX > 20)");
                println!("   Stop: Dynamic ATR-based trailing stop");
                println!("   Strategy: Let winners run!");
            }
            Signal::SellTrend => {
                println!("   ⛔ SELL TREND (Close Long)");
                println!("   Exit: ${:.2}", current_price);
                println!("   Reason: Uptrend broken (SuperTrend reversal)");
            }
            Signal::SellShort => {
                println!("   📉 SELL SHORT");
                println!("   Entry: ${:.2}", current_price);
                println!("   Reason: Strong downtrend detected (SuperTrend + ADX > 20)");
                println!("   Stop: Dynamic ATR-based trailing stop");
                println!("   Strategy: Profit from price decline!");
            }
            Signal::CoverShort => {
                println!("   🔼 COVER SHORT (Close Short)");
                println!("   Exit: ${:.2}", current_price);
                println!("   Reason: Downtrend broken (SuperTrend reversal)");
            }
            Signal::UpgradeToTrend => {
                println!("   ⬆️  UPGRADE TO TREND");
                println!("   Action: Keep position, switch to trend following");
                println!("   Reason: ADX increased, market becoming trendy");
            }
            Signal::Hold => {
                println!("   ⏸️  HOLD");
                println!("   Action: No action, waiting for setup");
            }
        }

        if signal != Signal::Hold {
            println!("\n   ⚠️  MODE: DRY RUN - Signal displayed only, no execution");
        }
    }
}

/// Fonction publique pour lancer le bot de trading live
pub async fn run_live_trading() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = HyperliquidFeed::new(COIN.to_string(), CANDLE_INTERVAL.to_string());
    feed.connect_and_trade().await
}
