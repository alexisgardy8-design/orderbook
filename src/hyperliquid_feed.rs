// 🚀 Hyperliquid WebSocket Feed - Live Trading Bot
// Connexion au DEX Hyperliquid pour récupérer les données SOL-PERP en temps réel
// Calcul des indicateurs (ADX, SuperTrend, Bollinger) et exécution d'ordres rapides

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use crate::position_manager::PositionState;

const HYPERLIQUID_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const COIN: &str = "SOL";
const CANDLE_INTERVAL: &str = "1h";  // 1 heure pour production
const CANDLE_BUFFER_SIZE: usize = 100;
const INITIAL_BANKROLL_USDC: f64 = 10000.0; // À remplacer par fetch réel

/// Bougie OHLCV de Hyperliquid (avec prices en f64 pour faciliter les calculs)
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

/// Client WebSocket pour Hyperliquid avec gestion de positions
pub struct HyperliquidFeed {
    coin: String,
    interval: String,
    candle_buffer: VecDeque<HyperCandle>,
    strategy: crate::adaptive_strategy::AdaptiveStrategy,
    position_manager: crate::position_manager::PositionManager,
    order_simulator: crate::order_executor::OrderSimulator,
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
            position_manager: crate::position_manager::PositionManager::new(INITIAL_BANKROLL_USDC),
            order_simulator: crate::order_executor::OrderSimulator::new(),
        }
    }

    /// Récupère la bankroll réelle de l'utilisateur (via API Hyperliquid)
    async fn fetch_user_bankroll(&self) -> Result<f64, Box<dyn std::error::Error>> {
        // Dans un vrai système, cela ferait appel à l'endpoint /info de Hyperliquid
        // Pour maintenant, on retourne la bankroll initiale
        Ok(INITIAL_BANKROLL_USDC)
    }

    /// Connexion au WebSocket et trading live
    pub async fn connect_and_trade(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║  🚀 HYPERLIQUID LIVE TRADING BOT - ADAPTIVE STRATEGY          ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        
        // Récupérer la bankroll de l'utilisateur
        let user_bankroll = match self.fetch_user_bankroll().await {
            Ok(balance) => balance,
            Err(_) => INITIAL_BANKROLL_USDC,
        };

        self.position_manager.bankroll.total_balance = user_bankroll;
        self.position_manager.bankroll.available_balance = user_bankroll;

        println!("⚙️  Configuration:");
        println!("   DEX:              Hyperliquid");
        println!("   Pair:             {}-PERP", self.coin);
        println!("   Timeframe:        {}", self.interval);
        println!("   Strategy:         Adaptive Bidirectional (Long + Short)");
        println!("   ADX Threshold:    20.0");
        println!("   Risk per Trade:   2% max loss");
        println!("   Bankroll (USDC):  ${:.2}", user_bankroll);
        println!("   Mode:             🔴 DRY RUN (signaux uniquement)\n");

        println!("🌐 Connecting to Hyperliquid WebSocket...");
        let (ws_stream, _) = connect_async(HYPERLIQUID_WS_URL).await?;
        println!("✅ Connected to {}\n", HYPERLIQUID_WS_URL);

        let (mut write, mut read) = ws_stream.split();

        // S'abonner aux bougies pour SOL-PERP
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
            
            // Mettre à jour le P&L actuel si position ouverte
            self.position_manager.update_current_pnl(candle.c);
            
            // Traiter les signaux de trading
            self.handle_trading_signal(signal, candle.c, candle.t);
            
            // Afficher l'état de la position
            self.display_position_status();
        } else {
            println!("\n⏳ Warming up indicators... ({}/50 candles)", self.candle_buffer.len());
        }
        
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }

    /// Traite les signaux de trading et exécute les ordres (simulés en DRY RUN)
    fn handle_trading_signal(
        &mut self,
        signal: crate::adaptive_strategy::Signal,
        current_price: f64,
        current_time: u64,
    ) {
        use crate::adaptive_strategy::Signal;
        use crate::position_manager::PositionState;

        match signal {
            Signal::BuyRange | Signal::BuyTrend => {
                if self.position_manager.position.is_none() {
                    // Calculer le SL à 2% en dessous du prix d'entrée
                    let stop_loss_price = current_price * 0.98;
                    
                    if let Some(position) = self.position_manager.open_long(current_price, current_time, stop_loss_price) {
                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     🟢 BUY (LONG)");
                        println!("   Entry:      ${:.2}", position.entry_price);
                        println!("   Size:       {:.4} SOL", position.position_size);
                        println!("   Value:      ${:.2}", position.position_value);
                        println!("   SL Price:   ${:.2} (-2%)", position.stop_loss_price);
                        println!("   Available:  ${:.2}", self.position_manager.bankroll.available_balance);
                        println!("   ⚠️  Mode: DRY RUN - Position simulated only");
                    }
                }
            }
            Signal::SellRange | Signal::SellTrend => {
                if let Some(pos) = &self.position_manager.position {
                    if pos.state == PositionState::Long {
                        if let Some(closed) = self.position_manager.close_position(current_price, current_time) {
                            println!("\n💰 TRADE EXECUTION:");
                            println!("   Action:     🔴 SELL (CLOSE LONG)");
                            println!("   Exit:       ${:.2}", closed.exit_price);
                            println!("   Size:       {:.4} SOL", closed.position_size);
                            println!("   P&L:        ${:+.2} ({:+.1}%)", closed.profit_loss, closed.profit_loss_pct);
                            println!("   Balance:    ${:.2}", self.position_manager.bankroll.total_balance);
                            println!("   ⚠️  Mode: DRY RUN - Position closed simulated only");
                        }
                    }
                }
            }
            Signal::SellShort => {
                if self.position_manager.position.is_none() {
                    // Calculer le SL à 2% au-dessus du prix d'entrée (pour un short)
                    let stop_loss_price = current_price * 1.02;
                    
                    if let Some(position) = self.position_manager.open_short(current_price, current_time, stop_loss_price) {
                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     📉 SHORT");
                        println!("   Entry:      ${:.2}", position.entry_price);
                        println!("   Size:       {:.4} SOL", position.position_size);
                        println!("   Value:      ${:.2}", position.position_value);
                        println!("   SL Price:   ${:.2} (+2%)", position.stop_loss_price);
                        println!("   Available:  ${:.2}", self.position_manager.bankroll.available_balance);
                        println!("   ⚠️  Mode: DRY RUN - Position simulated only");
                    }
                }
            }
            Signal::CoverShort => {
                if let Some(pos) = &self.position_manager.position {
                    if pos.state == PositionState::Short {
                        if let Some(closed) = self.position_manager.close_position(current_price, current_time) {
                            println!("\n💰 TRADE EXECUTION:");
                            println!("   Action:     🔼 COVER SHORT");
                            println!("   Exit:       ${:.2}", closed.exit_price);
                            println!("   Size:       {:.4} SOL", closed.position_size);
                            println!("   P&L:        ${:+.2} ({:+.1}%)", closed.profit_loss, closed.profit_loss_pct);
                            println!("   Balance:    ${:.2}", self.position_manager.bankroll.total_balance);
                            println!("   ⚠️  Mode: DRY RUN - Position closed simulated only");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Affiche l'état actuel de la position
    fn display_position_status(&self) {
        println!("\n📊 POSITION STATUS:");
        
        if let Some(pos) = &self.position_manager.position {
            let state_str = match pos.state {
                PositionState::Long => "🟢 LONG",
                PositionState::Short => "📉 SHORT",
                PositionState::None => "⚪ NONE",
            };
            
            println!("   State:        {}", state_str);
            println!("   Entry Price:  ${:.2}", pos.entry_price);
            println!("   Size:         {:.4} SOL", pos.position_size);
            println!("   SL Price:     ${:.2}", pos.stop_loss_price);
            println!("   P&L (unreal): ${:+.2} ({:+.1}%)", pos.unrealized_pnl, pos.unrealized_pnl_pct);
            
            // Vérifier si le SL est atteint
            if pos.is_stop_loss_hit(pos.unrealized_pnl) {
                println!("   ⚠️  STOP LOSS ALERT!");
            }
        } else {
            println!("   State:        ⚪ NO POSITION");
            println!("   Available:    ${:.2}", self.position_manager.bankroll.available_balance);
        }
        
        // Afficher les stats de trading
        let stats = self.position_manager.get_stats();
        println!("\n📈 TRADING STATS:");
        println!("   Total Trades:  {}", stats.total_trades);
        println!("   Win Rate:      {:.1}%", stats.win_rate);
        println!("   Total P&L:     ${:+.2}", stats.total_profit);
        println!("   Balance:       ${:.2}", stats.current_balance);
        println!("   Return:        {:+.1}%", stats.return_pct);
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

    /// Affiche le signal de trading généré
    fn display_signal(&self, signal: crate::adaptive_strategy::Signal) {
        use crate::adaptive_strategy::Signal;

        println!("   Signal: ", );
        
        match signal {
            Signal::BuyRange => println!("🟢 BUY RANGE (Long)"),
            Signal::SellRange => println!("🔴 SELL RANGE (Close Long)"),
            Signal::BuyTrend => println!("🚀 BUY TREND (Long Uptrend)"),
            Signal::SellTrend => println!("⛔ SELL TREND (Close Long)"),
            Signal::SellShort => println!("📉 SELL SHORT (Downtrend)"),
            Signal::CoverShort => println!("🔼 COVER SHORT (Close Short)"),
            Signal::UpgradeToTrend => println!("⬆️  UPGRADE TO TREND"),
            Signal::Hold => println!("⏸️  HOLD"),
        }
    }
}

/// Fonction publique pour lancer le bot de trading live
pub async fn run_live_trading() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = HyperliquidFeed::new(COIN.to_string(), CANDLE_INTERVAL.to_string());
    feed.connect_and_trade().await
}
