// 🚀 Hyperliquid WebSocket Feed - Live Trading Bot
// Connexion au DEX Hyperliquid pour récupérer les données SOL-PERP en temps réel
// Calcul des indicateurs (ADX, SuperTrend, Bollinger) et exécution d'ordres rapides

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::time::{sleep, Duration};
use crate::position_manager::PositionState;
use crate::hyperliquid_trade::HyperliquidTrader;
use crate::hyperliquid_historical::HyperliquidHistoricalData;

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

use tokio::sync::Mutex;

/// Client WebSocket pour Hyperliquid avec gestion de positions
pub struct HyperliquidFeed {
    coin: String,
    interval: String,
    pub candle_buffer: VecDeque<HyperCandle>,
    strategy: crate::adaptive_strategy::AdaptiveStrategy,
    position_manager: Arc<Mutex<crate::position_manager::PositionManager>>,
    order_simulator: crate::order_executor::OrderSimulator,
    pub trader: Option<HyperliquidTrader>,
    is_live: bool,
    telegram: Option<crate::telegram::TelegramBot>,
    is_running: Arc<AtomicBool>,
    last_processed_candle_t: u64,
}

impl HyperliquidFeed {
    /// Formatte un timestamp en date lisible
    fn format_timestamp(ts: u64) -> String {
        let secs = (ts / 1000) as i64;
        if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
            // Add 1 hour for France Winter Time
            let dt_paris = dt + chrono::Duration::hours(1);
            dt_paris.format("%Y-%m-%d %H:%M:%S (Paris)").to_string()
        } else {
            "Invalid timestamp".to_string()
        }
    }
    
    pub fn new(
        coin: String, 
        interval: String, 
        is_live: bool, 
        is_running: Arc<AtomicBool>,
        position_manager: Arc<Mutex<crate::position_manager::PositionManager>>,
        telegram: Option<crate::telegram::TelegramBot>
    ) -> Self {
        use crate::adaptive_strategy::AdaptiveConfig;
        
        // Always try to initialize trader to fetch balance/prices, even in DRY RUN
        let trader = match HyperliquidTrader::new() {
            Ok(t) => {
                if is_live {
                    println!("✅ LIVE TRADING ENABLED - Wallet: {}", t.wallet_address);
                } else {
                    println!("ℹ️  Hyperliquid Connection Active (Read-Only) - Wallet: {}", t.wallet_address);
                }
                Some(t)
            },
            Err(e) => {
                if is_live {
                    eprintln!("❌ Failed to initialize trader: {}", e);
                    eprintln!("⚠️  Falling back to DRY RUN mode");
                } else {
                    // Silent fail in dry run if keys are missing
                }
                None
            }
        };

        let is_trader_ready = trader.is_some();
        
        if telegram.is_some() {
            println!("✅ Telegram Notifications Enabled");
        } else {
            println!("⚠️  Telegram Notifications Disabled (Missing TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID)");
        }

        Self {
            coin,
            interval,
            candle_buffer: VecDeque::with_capacity(CANDLE_BUFFER_SIZE),
            strategy: crate::adaptive_strategy::AdaptiveStrategy::new(AdaptiveConfig {
                adx_threshold: 10.0,
                ..Default::default()
            }),
            position_manager,
            order_simulator: crate::order_executor::OrderSimulator::new(),
            trader,
            is_live: is_live && is_trader_ready, // Ensure is_live is false if trader init failed
            telegram,
            is_running,
            last_processed_candle_t: 0,
        }
    }

    /// Récupère la bankroll réelle de l'utilisateur (via API Hyperliquid)
    async fn fetch_user_bankroll(&self) -> Result<f64, Box<dyn std::error::Error>> {
        if let Some(trader) = &self.trader {
            println!("💰 Fetching real account balance from Hyperliquid...");
            match trader.get_account_balance().await {
                Ok(balance) => {
                    println!("✅ Balance fetched: ${:.2}", balance);
                    return Ok(balance);
                },
                Err(e) => {
                    eprintln!("❌ Failed to fetch balance: {}", e);
                    eprintln!("⚠️  Using default/fallback balance.");
                }
            }
        }
        
        Ok(INITIAL_BANKROLL_USDC)
    }

    /// Récupère les données historiques pour chauffer les indicateurs
    async fn warmup(&mut self) {
        println!("🔥 Warming up indicators with historical data...");
        
        let end_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        // 100 heures en arrière (pour être sûr d'avoir assez de données)
        let start_time = end_time - (100 * 60 * 60 * 1000);
        
        let fetcher = crate::hyperliquid_historical::HyperliquidHistoricalData::new(
            self.coin.clone(), 
            self.interval.clone()
        );

        // Exécuter dans un thread bloquant car ureq est synchrone
        let candles_result = tokio::task::spawn_blocking(move || {
            fetcher.fetch_candles(start_time, end_time).map_err(|e| e.to_string())
        }).await;

        match candles_result {
            Ok(Ok(candles)) => {
                println!("✅ Fetched {} historical candles for warmup", candles.len());
                
                for h_candle in candles {
                    // Conversion manuelle car les types sont différents (String vs f64)
                    if let Ok((o, h, l, c, v)) = h_candle.to_ohlc() {
                        let candle = HyperCandle {
                            t: h_candle.t,
                            close_t: h_candle.close_t,
                            s: h_candle.s,
                            i: h_candle.i,
                            o, h, l, c, v,
                            n: h_candle.n,
                        };
                        
                        // On utilise process_candle mais sans affichage pour le warmup
                        self.candle_buffer.push_back(candle.clone());
                        if self.candle_buffer.len() > CANDLE_BUFFER_SIZE {
                            self.candle_buffer.pop_front();
                        }
                        
                        // Update strategy state without triggering signals
                        self.strategy.update(candle.h, candle.l, candle.c);
                    }
                }
                println!("✅ Indicators warmed up! Buffer size: {}", self.candle_buffer.len());
                
                // FIX: On initialise last_processed_candle_t à l'avant-dernière bougie
                // pour être sûr que si la dernière bougie du buffer est la bougie en cours (ouverte),
                // elle sera bien traitée à sa fermeture.
                if self.candle_buffer.len() >= 2 {
                    let last_idx = self.candle_buffer.len() - 1;
                    // On prend l'avant-dernière bougie comme "dernière traitée"
                    // Comme ça, la dernière bougie (qui est peut-être en cours) sera considérée comme "nouvelle" à sa fermeture
                    self.last_processed_candle_t = self.candle_buffer[last_idx - 1].t;
                    println!("🔧 Last Processed Candle set to: {} (Penultimate)", self.last_processed_candle_t);
                } else if let Some(last) = self.candle_buffer.back() {
                    // Fallback si pas assez de bougies
                    self.last_processed_candle_t = last.t;
                }

                let last_price = self.candle_buffer.back().map(|c| c.c);
                self.display_indicators(last_price);

                // Update Shared State for Telegram immediately after warmup
                {
                    let mut pm = self.position_manager.lock().await;
                    pm.last_adx = self.strategy.get_adx_value();
                    pm.last_regime = format!("{:?}", self.strategy.get_current_regime());
                    pm.last_bollinger = self.strategy.get_bollinger_bands();
                }
            },
            Ok(Err(e)) => eprintln!("❌ Failed to fetch historical data: {}", e),
            Err(e) => eprintln!("❌ Task join error: {}", e),
        }
    }

    /// Connexion au WebSocket et trading live (avec reconnexion automatique)
    pub async fn connect_and_trade(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n╔════════════════════════════════════════════════════════════════╗");
        println!("║  🚀 HYPERLIQUID LIVE TRADING BOT - ADAPTIVE STRATEGY          ║");
        println!("╚════════════════════════════════════════════════════════════════╝\n");
        
        // Send initial control panel if Telegram is enabled
        if let Some(telegram) = &self.telegram {
             let _ = telegram.send_control_keyboard(true).await;
        }

        // Warmup indicators first (Une seule fois au démarrage)
        self.warmup().await;

        // Récupérer la bankroll de l'utilisateur
        let user_bankroll = match self.fetch_user_bankroll().await {
            Ok(balance) => balance,
            Err(_) => INITIAL_BANKROLL_USDC,
        };

        {
            let mut pm = self.position_manager.lock().await;
            pm.bankroll.total_balance = user_bankroll;
            pm.bankroll.available_balance = user_bankroll;
        }

        println!("⚙️  Configuration:");
        println!("   DEX:              Hyperliquid");
        println!("   Pair:             {}-PERP", self.coin);
        println!("   Timeframe:        {}", self.interval);
        println!("   Strategy:         Adaptive Bidirectional (Long + Short)");
        println!("   ADX Threshold:    20.0");
        println!("   Risk per Trade:   2% max loss");
        println!("   Bankroll (USDC):  ${:.2}", user_bankroll);
        if self.is_live {
            println!("   Mode:             🟢 LIVE TRADING (REAL MONEY)\n");
            
            // Set Leverage to 5x (Isolated)
            if let Some(trader) = &self.trader {
                println!("⚙️  Setting Leverage to 5x (Isolated)...");
                if let Err(e) = trader.update_leverage(&self.coin, 5, false).await {
                    eprintln!("⚠️  Failed to set leverage: {}", e);
                } else {
                    println!("✅ Leverage set to 5x");
                }
            }
        } else {
            println!("   Mode:             🔴 DRY RUN (signaux uniquement)\n");
        }

        // 📝 Log Startup to Supabase
        {
            let pm = self.position_manager.lock().await;
            if let Some(client) = &pm.supabase {
                let mode = if self.is_live { "LIVE TRADING" } else { "DRY RUN" };
                let log_msg = format!("Bot started - {} - {} {}", mode, self.coin, self.interval);
                let context = format!("Bankroll: ${:.2}", user_bankroll);
                
                let client_clone = client.clone();
                tokio::spawn(async move {
                    let _ = client_clone.log("INFO", &log_msg, Some(&context)).await;
                });
            }
        }

        // Boucle de reconnexion infinie
        loop {
            println!("🌐 Connecting to Hyperliquid WebSocket...");
            
            match self.run_websocket_session().await {
                Ok(_) => {
                    println!("⚠️ WebSocket session ended normally. Reconnecting in 5s...");
                }
                Err(e) => {
                    eprintln!("❌ WebSocket Error: {}. Reconnecting in 5s...", e);
                }
            }

            // Log disconnection
            {
                let pm = self.position_manager.lock().await;
                if let Some(client) = &pm.supabase {
                    let _ = client.log("WARN", "⚠️ Bot Disconnected - Attempting Reconnect...", None).await;
                }
            }

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    /// Session WebSocket unique (extrait de connect_and_trade)
    async fn run_websocket_session(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

        // S'abonner aux bougies 5m pour le monitoring intra-candle
        let subscribe_5m_msg = json!({
            "method": "subscribe",
            "subscription": {
                "type": "candle",
                "coin": self.coin,
                "interval": "5m"
            }
        });
        println!("📡 Subscribing to {} candles (5m) for intra-candle monitoring...", self.coin);
        write.send(Message::Text(subscribe_5m_msg.to_string())).await?;

        // Traiter les messages entrants
        let mut message_count = 0;
        let mut candle_count = 0;
        
        // Timer pour vérifier le changement d'heure et fetcher activement les bougies
        let mut check_interval = tokio::time::interval(Duration::from_secs(10));
        let mut last_checked_hour = 0u64;

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    // Vérifier si on a changé d'heure
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    
                    let current_hour = (now / 3600000) * 3600000; // Arrondir à l'heure pile
                    
                    // Si on vient de changer d'heure (et qu'on a initialisé)
                    if last_checked_hour != 0 && current_hour > last_checked_hour {
                        println!("\n⏰ Hour changed detected! Fetching last closed candle via REST API...");
                        println!("   Previous hour: {}", last_checked_hour);
                        println!("   Current hour:  {}", current_hour);
                        
                        // Fetcher la dernière bougie fermée (celle de last_checked_hour)
                        let historical = HyperliquidHistoricalData::new(self.coin.clone(), self.interval.clone());
                        
                        // Fetch la bougie entre last_checked_hour et current_hour
                        match historical.fetch_candles(last_checked_hour, current_hour) {
                            Ok(candles) => {
                                if let Some(closed_candle) = candles.first() {
                                    println!("✅ Fetched closed candle via REST: t={}", closed_candle.t);
                                    
                                    // Convertir en HyperCandle avec f64
                                    if let Ok((o, h, l, c, v)) = closed_candle.to_ohlc() {
                                        let candle_f64 = HyperCandle {
                                            t: closed_candle.t,
                                            close_t: closed_candle.close_t,
                                            s: closed_candle.s.clone(),
                                            i: closed_candle.i.clone(),
                                            o,
                                            c,
                                            h,
                                            l,
                                            v,
                                            n: closed_candle.n,
                                        };
                                        
                                        // Process comme si c'était venu du WebSocket
                                        candle_count += 1;
                                        self.process_candle(candle_f64, candle_count, true).await;
                                    }
                                }
                            },
                            Err(e) => eprintln!("⚠️ Failed to fetch closed candle: {}", e),
                        }
                    }
                    
                    // Mettre à jour le tracker d'heure
                    if last_checked_hour == 0 {
                        last_checked_hour = current_hour;
                    } else if current_hour > last_checked_hour {
                        last_checked_hour = current_hour;
                    }
                }
                
                message = read.next() => {
                    if let Some(message) = message {
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
                                                    if candle.i == self.interval {
                                                        // 🛡️ PROTECTION RECONNEXION
                                                        // Si la bougie reçue est plus vieille ou égale à la dernière traitée, on l'ignore
                                                        // SAUF si c'est la bougie en cours (t == last_processed) pour la mise à jour live
                                                        if candle.t < self.last_processed_candle_t {
                                                            continue;
                                                        }
                                                        
                                                        candle_count += 1;
                                                        self.process_candle(candle, candle_count, false).await;
                                                    } else if candle.i == "5m" {
                                                        self.process_5m_candle(candle).await;
                                                    }
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
                    } else {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    // Helper method for strategy execution
    async fn execute_strategy_logic(&mut self, closed_candle: HyperCandle, execution_price: f64, current_time: u64) {
        println!("   Close Time: {} (Signal Time: {})", closed_candle.t, current_time);
        println!("   Open:   ${:.2}", closed_candle.o);
        println!("   High:   ${:.2}", closed_candle.h);
        println!("   Low:    ${:.2}", closed_candle.l);
        println!("   Close:  ${:.2}", closed_candle.c);
        println!("   Volume: {:.2}", closed_candle.v);
        
        let change_pct = ((closed_candle.c - closed_candle.o) / closed_candle.o) * 100.0;
        let color = if change_pct > 0.0 { "🟢" } else { "🔴" };
        println!("   Change: {} {:+.2}%", color, change_pct);

        if self.candle_buffer.len() >= 50 {
            // 🛡️ KILL SWITCH CHECK (Critical Security #3)
            {
                let mut pm = self.position_manager.lock().await;
                let config = crate::adaptive_strategy::AdaptiveConfig::default();
                if let Err(e) = pm.check_safety_limits(config.max_daily_drawdown_pct, config.max_trades_per_hour) {
                    eprintln!("\n{}", e);
                    if let Some(bot) = &self.telegram {
                        let _ = bot.send_default_message_with_menu_btn(&e).await;
                    }
                    // Stop processing this candle to prevent new orders
                    return;
                }
            }

            // Mise à jour des indicateurs avec la bougie fermée
            let signal = self.strategy.update(closed_candle.h, closed_candle.l, closed_candle.c);
            
            println!("\n📊 STRATEGY ANALYSIS (Closed Candle):");
            self.display_indicators(Some(closed_candle.c));

            // Update Shared State for Telegram
            {
                let mut pm = self.position_manager.lock().await;
                pm.last_adx = self.strategy.get_adx_value();
                pm.last_regime = format!("{:?}", self.strategy.get_current_regime());
                pm.last_bollinger = self.strategy.get_bollinger_bands();
            }

            // 📝 Log to Supabase
            {
                let pm = self.position_manager.lock().await;
                if let Some(client) = &pm.supabase {
                    let log_msg = format!("Candle Closed: ${:.2} ({:+.2}%) - Signal: {:?}", closed_candle.c, change_pct, signal);
                    let context = format!("ADX: {:.2}, Regime: {:?}", self.strategy.get_adx_value(), self.strategy.get_current_regime());
                    // We spawn the log task to avoid blocking the strategy execution
                    let client_clone = client.clone();
                    tokio::spawn(async move {
                        let _ = client_clone.log("INFO", &log_msg, Some(&context)).await;
                    });
                }
            }
            
            // Mettre à jour le P&L actuel si position ouverte
            {
                let mut pm = self.position_manager.lock().await;
                pm.update_current_pnl(closed_candle.c);
            }
            
            // Traiter les signaux de trading
            self.handle_trading_signal(signal, execution_price, current_time).await;
            
            // Afficher l'état de la position
            self.display_position_status().await;
        } else {
            println!("\n⏳ Warming up indicators... ({}/50 candles)", self.candle_buffer.len());
        }
    }

    /// Traite une bougie 5m pour vérifier les conditions de sortie intra-candle
    async fn process_5m_candle(&mut self, candle: HyperCandle) {
        // On ne fait rien si aucune position n'est ouverte
        if self.strategy.get_position_type() == crate::adaptive_strategy::PositionType::None {
            return;
        }

        // ⚠️ IMPORTANT: On ne recalcule PAS l'ADX ni les indicateurs ici.
        // On vérifie UNIQUEMENT si le prix touche le niveau de sortie (Milieu Bollinger H1)
        // Cela garantit la stabilité de la stratégie (pas de sortie sur bruit 5m)
        
        // Check exit condition using the 5m candle High/Low
        if let Some(signal) = self.strategy.check_exit_condition(candle.h, candle.l, candle.c) {
            println!("\n⚡ INTRA-CANDLE EXIT TRIGGERED (5m Candle)!");
            println!("   Time: {} (Paris)", Self::format_timestamp(candle.t));
            println!("   Price hit target: High ${:.2} / Low ${:.2}", candle.h, candle.l);
            
            // Force exit in strategy state
            self.strategy.force_exit();
            
            // Execute signal immediately
            self.handle_trading_signal(signal, candle.c, candle.t).await;
        }
    }

    /// Traite une bougie reçue et génère des signaux UNIQUEMENT à la clôture
    pub async fn process_candle(&mut self, candle: HyperCandle, count: usize, force_close: bool) {
        let last_candle_time = self.candle_buffer.back().map(|c| c.t);

        // Cas spécial: Force Close (via Watchdog)
        if force_close {
            // On met à jour le buffer avec cette bougie finale
            if let Some(last_t) = last_candle_time {
                if candle.t == last_t {
                    self.candle_buffer.pop_back();
                }
            }
            self.candle_buffer.push_back(candle.clone());
            
            // Si cette bougie n'a pas encore été traitée comme "fermée"
            if candle.t > self.last_processed_candle_t {
                println!("\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("🕯️  CANDLE FORCE CLOSED (Watchdog) - {} {}", candle.s, self.interval);
                // Use Close as execution price since we don't have next Open
                self.execute_strategy_logic(candle.clone(), candle.c, candle.t).await; 
                self.last_processed_candle_t = candle.t;
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
            }
            return;
        }

        if let Some(last_t) = last_candle_time {
            if candle.t == last_t {
                // 🔄 UPDATE: La bougie est en cours de formation (même timestamp)
                // On met à jour la dernière bougie du buffer sans déclencher la stratégie
                self.candle_buffer.pop_back();
                self.candle_buffer.push_back(candle.clone());
                
                // Affichage discret pour le suivi live
                print!("\r⏳ Candle Update: ${:.2} (H: {:.2} L: {:.2})", candle.c, candle.h, candle.l);
                use std::io::Write;
                std::io::stdout().flush().unwrap();

                // ⚡ INTRA-CANDLE EXIT CHECK
                // Check if we hit the target (Middle Band) during this candle
                if let Some(signal) = self.strategy.check_exit_condition(candle.h, candle.l, candle.c) {
                    println!("\n\n⚡ INTRA-CANDLE EXIT TRIGGERED!");
                    println!("   Price hit target: High ${:.2}", candle.h);
                    
                    // Force exit in strategy state
                    self.strategy.force_exit();
                    
                    // Execute signal immediately
                    self.handle_trading_signal(signal, candle.c, candle.t).await;
                }
                
            } else if candle.t > last_t {
                // 🏁 CLÔTURE: Une nouvelle bougie commence, la précédente est terminée
                // IMPORTANT: La bougie 'candle' est la NOUVELLE bougie (t+1)
                // La bougie qui vient de fermer est la dernière du buffer (t)
                
                println!("\n\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!("🕯️  CANDLE CLOSED - {} {}", candle.s, self.interval);
                
                // Récupérer la bougie qui vient de fermer (la dernière du buffer)
                let closed_candle = self.candle_buffer.back().unwrap().clone();
                
                // Vérification de latence
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                let latency = now.saturating_sub(candle.t); // Différence entre maintenant et le début de la nouvelle bougie
                if latency > 5000 {
                    println!("⚠️  WARNING: High Latency detected! Candle closed {}ms ago.", latency);
                }

                println!("   Close Time: {} (New Open: {})", closed_candle.t, candle.t);
                
                if closed_candle.t > self.last_processed_candle_t {
                    // Use Open of new candle as execution price
                    self.execute_strategy_logic(closed_candle.clone(), candle.o, candle.t).await;
                    self.last_processed_candle_t = closed_candle.t;
                } else {
                    println!("⚠️  Candle {} already processed. Skipping strategy execution.", closed_candle.t);
                }
                
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

                // Ajouter la NOUVELLE bougie au buffer
                self.candle_buffer.push_back(candle.clone());
                if self.candle_buffer.len() > CANDLE_BUFFER_SIZE {
                    self.candle_buffer.pop_front();
                }
            }
        } else {
            // Premier ajout (Buffer vide)
            self.candle_buffer.push_back(candle);
        }
    }

    /// Traite les signaux de trading et exécute les ordres (simulés en DRY RUN)
    async fn handle_trading_signal(
        &mut self,
        signal: crate::adaptive_strategy::Signal,
        current_price: f64,
        current_time: u64,
    ) {
        use crate::adaptive_strategy::Signal;
        use crate::position_manager::PositionState;

        // Check if bot is running (Telegram Control)
        if !self.is_running.load(Ordering::SeqCst) {
            if matches!(signal, Signal::Hold) { return; }
            println!("⏸️  Bot is PAUSED. Ignoring signal: {:?}", signal);
            return;
        }

        match signal {
            Signal::BuyRange | Signal::BuyTrend => {
                let mut pm = self.position_manager.lock().await;
                if pm.position.is_none() {
                    // Calculer le SL à 5% en dessous du prix d'entrée (Optimized Strategy)
                    let stop_loss_price = current_price * 0.95;
                    
                    if let Some(mut position) = pm.open_long(current_price, current_time, stop_loss_price) {
                        
                        if self.is_live {
                            if let Some(trader) = &self.trader {
                                println!("🚀 EXECUTING LIVE ORDER...");
                                // Use Market Order (Limit with 5% slippage)
                                match trader.place_market_order_with_retry(&self.coin, true, position.position_size, current_price, 0.05, 3).await {
                                    Ok(oid) => {
                                        println!("✅ LIVE ORDER PLACED: OID {}", oid);
                                        
                                        // Wait for fill to get real price
                                        println!("⏳ Waiting for fill details...");
                                        sleep(Duration::from_secs(2)).await;
                                        
                                        // Fetch real fill data
                                        if let Ok(fills) = trader.get_user_fills().await {
                                            // Find the fill for this order (approximate by time and coin)
                                            let recent_fill = fills.iter().find(|f| 
                                                f.coin == self.coin && 
                                                f.side == "B" && 
                                                f.time > (current_time - 10000)
                                            );
                                            
                                            if let Some(fill) = recent_fill {
                                                let real_price = fill.px.parse::<f64>().unwrap_or(current_price);
                                                let real_fee = fill.fee.parse::<f64>().unwrap_or(0.0);
                                                
                                                println!("📝 Real Fill: ${:.2} (Fee: ${:.4})", real_price, real_fee);
                                                
                                                // Update position with real data
                                                if let Some(pos) = &mut pm.position {
                                                    pos.entry_price = real_price;
                                                    pos.entry_fee = real_fee;
                                                    // Recalculate SL based on real entry
                                                    let sl_pct = 0.05; // 5% SL
                                                    pos.stop_loss_price = real_price * (1.0 - sl_pct);
                                                    pos.stop_loss_pct = sl_pct * 100.0;
                                                    position.entry_price = real_price; // Update local copy for display
                                                    position.stop_loss_price = pos.stop_loss_price;
                                                }
                                            }
                                        }
                                        
                                        // 🛡️ INTEGRATED STOP LOSS for LONG
                                        // SL is below entry price
                                        let sl_price = position.stop_loss_price;
                                        let sl_price = (sl_price * 100.0).round() / 100.0;
                                        
                                        println!("🛡️ PLACING STOP LOSS @ ${:.2} (-5%)...", sl_price);
                                        match trader.place_stop_loss_order(&self.coin, false, sl_price, position.position_size).await {
                                            Ok(sl_oid) => println!("✅ STOP LOSS PLACED: OID {}", sl_oid),
                                            Err(e) => eprintln!("❌ STOP LOSS FAILED: {}", e),
                                        }
                                    },
                                    Err(e) => eprintln!("❌ LIVE ORDER FAILED: {}", e),
                                }
                            }
                        } else {
                            println!("   ⚠️  Mode: DRY RUN - Position simulated only");
                        }

                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     🟢 BUY (LONG)");
                        println!("   Entry:      ${:.2}", position.entry_price);
                        println!("   Size:       {:.4} SOL", position.position_size);
                        println!("   Value:      ${:.2}", position.position_value);
                        println!("   SL Price:   ${:.2} (-5%)", position.stop_loss_price);
                        println!("   Available:  ${:.2}", pm.bankroll.available_balance);

                        // 💾 Save to Supabase
                        let supabase_client = pm.supabase.clone();
                        if let Some(client) = supabase_client {
                            let db_pos = crate::supabase::DbPosition {
                                id: None,
                                coin: self.coin.clone(),
                                side: "LONG".to_string(),
                                entry_price: position.entry_price,
                                size: position.position_size,
                                status: "OPEN".to_string(),
                                created_at: None,
                                closed_at: None,
                                exit_price: None,
                                pnl: None,
                            };
                            
                            match client.save_position(&db_pos).await {
                                Ok(id) => {
                                    if let Some(pos) = &mut pm.position {
                                        pos.db_id = Some(id);
                                    }
                                    println!("✅ Position saved to Supabase (ID: {})", id);
                                },
                                Err(e) => eprintln!("❌ Failed to save position to Supabase: {}", e),
                            }
                        }
                    }
                }
            }
            Signal::SellRange | Signal::SellTrend => {
                let mut pm = self.position_manager.lock().await;
                let mut should_close = false;
                let mut entry_fee = 0.0;
                let mut entry_time = 0;
                
                if let Some(pos) = &pm.position {
                    if pos.state == PositionState::Long {
                        should_close = true;
                        entry_fee = pos.entry_fee;
                        entry_time = pos.entry_time;
                    }
                }

                if should_close {
                    if let Some(closed) = pm.close_position(current_price, current_time) {
                        
                        let mut final_exit_price = closed.exit_price;
                        let mut final_pnl = closed.profit_loss;
                        let mut final_net_pnl = closed.profit_loss;
                        let mut is_real_data = false;

                        if self.is_live {
                            if let Some(trader) = &self.trader {
                                println!("🚀 EXECUTING LIVE ORDER...");
                                // Use Market Order to close position
                                match trader.place_market_order_with_retry(&self.coin, false, closed.position_size, current_price, 0.05, 10).await {
                                    Ok(oid) => {
                                        println!("✅ LIVE ORDER PLACED: OID {}", oid);
                                        
                                        println!("⏳ Waiting for fill details...");
                                        sleep(Duration::from_secs(2)).await;

                                        // Fetch real data
                                        let mut realized_pnl = 0.0;
                                        let mut closing_fee = 0.0;
                                        let mut funding_paid = 0.0;

                                        if let Ok(fills) = trader.get_user_fills().await {
                                            let recent_fill = fills.iter().find(|f| 
                                                f.coin == self.coin && 
                                                f.side == "A" && // Sell
                                                f.time > (current_time - 10000)
                                            );
                                            
                                            if let Some(fill) = recent_fill {
                                                final_exit_price = fill.px.parse().unwrap_or(closed.exit_price);
                                                closing_fee = fill.fee.parse().unwrap_or(0.0);
                                                if let Some(pnl_str) = &fill.closed_pnl {
                                                    realized_pnl = pnl_str.parse().unwrap_or(0.0);
                                                } else {
                                                    realized_pnl = (final_exit_price - closed.entry_price) * closed.position_size;
                                                }
                                                is_real_data = true;
                                            }
                                        }

                                        // Fetch funding
                                        if let Ok(fundings) = trader.get_user_funding(entry_time).await {
                                            for f in fundings {
                                                if f.coin == self.coin {
                                                    let amount = f.usdc.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                    funding_paid += amount;
                                                }
                                            }
                                        }

                                        final_pnl = realized_pnl;
                                        final_net_pnl = realized_pnl - closing_fee - entry_fee + funding_paid;
                                        
                                        println!("📝 Real Data: Exit=${:.2}, PnL=${:.2}, Fee=${:.4}, Funding=${:.4}", 
                                            final_exit_price, realized_pnl, closing_fee + entry_fee, funding_paid);
                                    },
                                    Err(e) => eprintln!("❌ LIVE ORDER FAILED: {}", e),
                                }
                            }
                        } else {
                            println!("   ⚠️  Mode: DRY RUN - Position closed simulated only");
                            let estimated_fee = closed.position_size * closed.exit_price * 0.0005 * 2.0;
                            final_net_pnl = closed.profit_loss - estimated_fee;
                        }

                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     🔴 SELL (CLOSE LONG)");
                        println!("   Exit:       ${:.2}", final_exit_price);
                        println!("   Size:       {:.4} SOL", closed.position_size);
                        println!("   P&L:        ${:+.2}", final_pnl);
                        println!("   Net P&L:    ${:+.2}", final_net_pnl);
                        println!("   Balance:    ${:.2}", pm.bankroll.total_balance);
                        
                        // � Update Supabase
                        if let Some(id) = closed.db_id {
                            let supabase_client = pm.supabase.clone();
                            if let Some(client) = supabase_client {
                                let update = crate::supabase::DbPosition {
                                    id: Some(id),
                                    coin: self.coin.clone(),
                                    side: "LONG".to_string(),
                                    entry_price: closed.entry_price,
                                    size: closed.position_size,
                                    status: "CLOSED".to_string(),
                                    created_at: None,
                                    closed_at: Some(chrono::Utc::now()),
                                    exit_price: Some(final_exit_price),
                                    pnl: Some(final_net_pnl),
                                };
                                
                                let _ = client.update_position(id, &update).await;
                                println!("✅ Position updated in Supabase (ID: {})", id);
                            }
                        }

                        // �📱 Telegram Notification
                        if let Some(telegram) = &self.telegram {
                            let pnl_emoji = if final_net_pnl >= 0.0 { "🟢" } else { "🔴" };
                            let pnl_type = if is_real_data { "Real" } else { "Est" };
                            
                            let message = format!(
                                "💰 *Position Closed*\n\n\
                                Action: 🔴 SELL (CLOSE LONG)\n\
                                Exit Price: ${:.2}\n\
                                Size: {:.4} SOL\n\
                                Gross P&L: ${:+.2}\n\
                                Net P&L ({}): {} ${:+.2} ({:+.2}%)\n\
                                Balance: ${:.2}",
                                final_exit_price,
                                closed.position_size,
                                final_pnl,
                                pnl_type, pnl_emoji, final_net_pnl, (final_net_pnl / (closed.position_size * closed.entry_price)) * 100.0,
                                pm.bankroll.total_balance
                            );
                            
                            let _ = telegram.send_message(&message).await;
                        }
                    }
                }
            }
            Signal::SellShort => {
                let mut pm = self.position_manager.lock().await;
                if pm.position.is_none() {
                    // Calculer le SL à 5% au-dessus du prix d'entrée (pour un short)
                    let stop_loss_price = current_price * 1.05;
                    
                    if let Some(mut position) = pm.open_short(current_price, current_time, stop_loss_price) {
                        
                        if self.is_live {
                            if let Some(trader) = &self.trader {
                                println!("🚀 EXECUTING LIVE ORDER...");
                                // Use Market Order (Limit with 5% slippage)
                                match trader.place_market_order(&self.coin, false, position.position_size, current_price, 0.05).await {
                                    Ok(oid) => {
                                        println!("✅ LIVE ORDER PLACED: OID {}", oid);
                                        
                                        println!("⏳ Waiting for fill details...");
                                        sleep(Duration::from_secs(2)).await;
                                        
                                        if let Ok(fills) = trader.get_user_fills().await {
                                            let recent_fill = fills.iter().find(|f| 
                                                f.coin == self.coin && 
                                                f.side == "A" && // Sell Short
                                                f.time > (current_time - 10000)
                                            );
                                            
                                            if let Some(fill) = recent_fill {
                                                let real_price = fill.px.parse::<f64>().unwrap_or(current_price);
                                                let real_fee = fill.fee.parse::<f64>().unwrap_or(0.0);
                                                
                                                println!("📝 Real Fill: ${:.2} (Fee: ${:.4})", real_price, real_fee);
                                                
                                                if let Some(pos) = &mut pm.position {
                                                    pos.entry_price = real_price;
                                                    pos.entry_fee = real_fee;
                                                    let sl_pct = 0.05; // 5% SL
                                                    pos.stop_loss_price = real_price * (1.0 + sl_pct);
                                                    pos.stop_loss_pct = sl_pct * 100.0;
                                                    position.entry_price = real_price;
                                                    position.stop_loss_price = pos.stop_loss_price;
                                                }
                                            }
                                        }
                                        
                                        // 🛡️ INTEGRATED STOP LOSS for SHORT
                                        // SL is above entry price
                                        let sl_price = position.stop_loss_price;
                                        let sl_price = (sl_price * 100.0).round() / 100.0;
                                        
                                        println!("🛡️ PLACING STOP LOSS @ ${:.2} (+5%)...", sl_price);
                                        match trader.place_stop_loss_order(&self.coin, true, sl_price, position.position_size).await {
                                            Ok(sl_oid) => println!("✅ STOP LOSS PLACED: OID {}", sl_oid),
                                            Err(e) => eprintln!("❌ STOP LOSS FAILED: {}", e),
                                        }
                                    },
                                    Err(e) => eprintln!("❌ LIVE ORDER FAILED: {}", e),
                                }
                            }
                        } else {
                            println!("   ⚠️  Mode: DRY RUN - Position simulated only");
                        }

                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     📉 SHORT");
                        println!("   Entry:      ${:.2}", position.entry_price);
                        println!("   Size:       {:.4} SOL", position.position_size);
                        println!("   Value:      ${:.2}", position.position_value);
                        println!("   SL Price:   ${:.2} (+6%)", position.stop_loss_price);
                        println!("   Available:  ${:.2}", pm.bankroll.available_balance);

                        // 💾 Save to Supabase
                        let supabase_client = pm.supabase.clone();
                        if let Some(client) = supabase_client {
                            let db_pos = crate::supabase::DbPosition {
                                id: None,
                                coin: self.coin.clone(),
                                side: "SHORT".to_string(),
                                entry_price: position.entry_price,
                                size: position.position_size,
                                status: "OPEN".to_string(),
                                created_at: None,
                                closed_at: None,
                                exit_price: None,
                                pnl: None,
                            };
                            
                            match client.save_position(&db_pos).await {
                                Ok(id) => {
                                    if let Some(pos) = &mut pm.position {
                                        pos.db_id = Some(id);
                                    }
                                    println!("✅ Position saved to Supabase (ID: {})", id);
                                },
                                Err(e) => eprintln!("❌ Failed to save position to Supabase: {}", e),
                            }
                        }
                    }
                }
            }
            Signal::CoverShort => {
                let mut pm = self.position_manager.lock().await;
                let mut should_close = false;
                let mut entry_fee = 0.0;
                let mut entry_time = 0;

                if let Some(pos) = &pm.position {
                    if pos.state == PositionState::Short {
                        should_close = true;
                        entry_fee = pos.entry_fee;
                        entry_time = pos.entry_time;
                    }
                }

                if should_close {
                    if let Some(closed) = pm.close_position(current_price, current_time) {
                        
                        let mut final_exit_price = closed.exit_price;
                        let mut final_pnl = closed.profit_loss;
                        let mut final_net_pnl = closed.profit_loss;
                        let mut is_real_data = false;

                        if self.is_live {
                            if let Some(trader) = &self.trader {
                                println!("🚀 EXECUTING LIVE ORDER...");
                                // Use Market Order to close position
                                match trader.place_market_order(&self.coin, true, closed.position_size, current_price, 0.05).await {
                                    Ok(oid) => {
                                        println!("✅ LIVE ORDER PLACED: OID {}", oid);
                                        
                                        println!("⏳ Waiting for fill details...");
                                        sleep(Duration::from_secs(2)).await;

                                        let mut realized_pnl = 0.0;
                                        let mut closing_fee = 0.0;
                                        let mut funding_paid = 0.0;

                                        if let Ok(fills) = trader.get_user_fills().await {
                                            let recent_fill = fills.iter().find(|f| 
                                                f.coin == self.coin && 
                                                f.side == "B" && // Buy to Cover
                                                f.time > (current_time - 10000)
                                            );
                                            
                                            if let Some(fill) = recent_fill {
                                                final_exit_price = fill.px.parse().unwrap_or(closed.exit_price);
                                                closing_fee = fill.fee.parse().unwrap_or(0.0);
                                                if let Some(pnl_str) = &fill.closed_pnl {
                                                    realized_pnl = pnl_str.parse().unwrap_or(0.0);
                                                } else {
                                                    realized_pnl = (closed.entry_price - final_exit_price) * closed.position_size;
                                                }
                                                is_real_data = true;
                                            }
                                        }

                                        if let Ok(fundings) = trader.get_user_funding(entry_time).await {
                                            for f in fundings {
                                                if f.coin == self.coin {
                                                    let amount = f.usdc.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                    funding_paid += amount;
                                                }
                                            }
                                        }

                                        final_pnl = realized_pnl;
                                        final_net_pnl = realized_pnl - closing_fee - entry_fee + funding_paid;
                                        
                                        println!("📝 Real Data: Exit=${:.2}, PnL=${:.2}, Fee=${:.4}, Funding=${:.4}", 
                                            final_exit_price, realized_pnl, closing_fee + entry_fee, funding_paid);
                                    },
                                    Err(e) => eprintln!("❌ LIVE ORDER FAILED: {}", e),
                                }
                            }
                        } else {
                            println!("   ⚠️  Mode: DRY RUN - Position simulated only");
                            let estimated_fee = closed.position_size * closed.exit_price * 0.0005 * 2.0;
                            final_net_pnl = closed.profit_loss - estimated_fee;
                        }

                        println!("\n💰 TRADE EXECUTION:");
                        println!("   Action:     🔼 COVER SHORT");
                        println!("   Exit:       ${:.2}", final_exit_price);
                        println!("   Size:       {:.4} SOL", closed.position_size);
                        println!("   P&L:        ${:+.2}", final_pnl);
                        println!("   Net P&L:    ${:+.2}", final_net_pnl);
                        println!("   Balance:    ${:.2}", pm.bankroll.total_balance);
                        
                        // � Update Supabase
                        if let Some(id) = closed.db_id {
                            let supabase_client = pm.supabase.clone();
                            if let Some(client) = supabase_client {
                                let update = crate::supabase::DbPosition {
                                    id: Some(id),
                                    coin: self.coin.clone(),
                                    side: "SHORT".to_string(),
                                    entry_price: closed.entry_price,
                                    size: closed.position_size,
                                    status: "CLOSED".to_string(),
                                    created_at: None,
                                    closed_at: Some(chrono::Utc::now()),
                                    exit_price: Some(final_exit_price),
                                    pnl: Some(final_net_pnl),
                                };
                                
                                let _ = client.update_position(id, &update).await;
                                println!("✅ Position updated in Supabase (ID: {})", id);
                            }
                        }

                        // �📱 Telegram Notification
                        if let Some(telegram) = &self.telegram {
                            let pnl_emoji = if final_net_pnl >= 0.0 { "🟢" } else { "🔴" };
                            let pnl_type = if is_real_data { "Real" } else { "Est" };
                            
                            let message = format!(
                                "💰 *Position Closed*\n\n\
                                Action: 🔼 COVER SHORT\n\
                                Exit Price: ${:.2}\n\
                                Size: {:.4} SOL\n\
                                Gross P&L: ${:+.2}\n\
                                Net P&L ({}): {} ${:+.2} ({:+.2}%)\n\
                                Balance: ${:.2}",
                                final_exit_price,
                                closed.position_size,
                                final_pnl,
                                pnl_type, pnl_emoji, final_net_pnl, (final_net_pnl / (closed.position_size * closed.entry_price)) * 100.0,
                                pm.bankroll.total_balance
                            );
                            
                            let _ = telegram.send_message(&message).await;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Affiche l'état actuel de la position
    async fn display_position_status(&self) {
        println!("\n📊 POSITION STATUS:");
        
        let pm = self.position_manager.lock().await;
        if let Some(pos) = &pm.position {
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
            println!("   Available:    ${:.2}", pm.bankroll.available_balance);
        }
        
        // Afficher les stats de trading
        let stats = pm.get_stats();
        println!("\n📈 TRADING STATS:");
        println!("   Total Trades:  {}", stats.total_trades);
        println!("   Win Rate:      {:.1}%", stats.win_rate);
        println!("   Total P&L:     ${:+.2}", stats.total_profit);
        println!("   Balance:       ${:.2}", stats.current_balance);
        println!("   Return:        {:+.1}%", stats.return_pct);
    }

    /// Affiche les indicateurs calculés
    fn display_indicators(&self, current_price: Option<f64>) {
        use crate::adaptive_strategy::MarketRegime;
        
        let regime = self.strategy.get_current_regime();
        let adx = self.strategy.get_adx_value();
        let position = self.strategy.get_position_type();
        let bollinger = self.strategy.get_bollinger_bands();

        println!("   ADX Value:      {:.2}", adx);
        println!("   Market Regime:  {:?}", regime);
        println!("   Position Type:  {:?}", position);
        
        match regime {
            MarketRegime::Ranging => {
                println!("   Mode:           🎯 RANGE (Bollinger Mean Reversion)");
                if let Some((lower, middle, upper)) = bollinger {
                    println!("   Bollinger Bands (H1):");
                    println!("     Upper: ${:.2}", upper);
                    println!("     Middle: ${:.2}", middle);
                    println!("     Lower: ${:.2}", lower);
                    
                    if let Some(price) = current_price {
                        if price > upper {
                            println!("     Status: 🔴 PRICE ABOVE UPPER BAND (Overbought)");
                        } else if price < lower {
                            println!("     Status: 🟢 PRICE BELOW LOWER BAND (Oversold)");
                        } else {
                            println!("     Status: ⚪ PRICE INSIDE BANDS");
                        }
                    }
                }
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
    let is_live = std::env::var("LIVE_TRADING").unwrap_or_else(|_| "false".to_string()) == "true";
    let is_running = Arc::new(AtomicBool::new(true)); // Shared state across reconnections
    
    // Initialize Supabase
    let supabase = crate::supabase::SupabaseClient::new();
    if supabase.is_some() {
        println!("✅ Supabase Logging Enabled");
    } else {
        println!("⚠️  Supabase Logging Disabled (Missing SUPABASE_URL or SUPABASE_KEY)");
    }

    // Initialize shared resources
    let position_manager = Arc::new(Mutex::new(crate::position_manager::PositionManager::new(INITIAL_BANKROLL_USDC, supabase.clone())));
    
    // Load existing positions from Supabase
    {
        let mut pm = position_manager.lock().await;
        pm.init().await;
    }

    let telegram = crate::telegram::TelegramBot::new();

    // Handle Shutdown Signal
    let ir_signal = is_running.clone();
    let tg_signal = telegram.clone();
    let sb_signal = supabase.clone();
    
    tokio::spawn(async move {
        #[cfg(unix)]
        let mut sig_term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        #[cfg(unix)]
        let mut sig_hup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()).unwrap();

        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Shutdown signal (Ctrl+C) received.");
            }
            _ = sig_term.recv() => {
                println!("\n🛑 Shutdown signal (SIGTERM) received.");
            }
            _ = sig_hup.recv() => {
                println!("\n🛑 Shutdown signal (SIGHUP) received.");
            }
        }

        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;

        println!("Stopping bot...");
        ir_signal.store(false, Ordering::SeqCst);
        
        if let Some(bot) = tg_signal {
            let _ = bot.send_message("🛑 *Bot Stopping* - Shutdown signal received.").await;
        }
        
        if let Some(client) = sb_signal {
            let _ = client.log("INFO", "Bot stopping - Shutdown signal", None).await;
        }

        // Force exit process immediately
        std::process::exit(0);
    });

    // Start Telegram Listener ONCE
    if let Some(bot) = telegram.clone() {
        let pm = position_manager.clone();
        let ir = is_running.clone();
        println!("📱 Starting Telegram Listener (Background Task)...");
        
        // Send Startup Message
        let _ = bot.send_default_message_with_menu_btn("🟢 *Bot Connected* - System Online").await;

        tokio::spawn(async move {
            bot.run_listener(ir, pm).await;
        });
    }

    loop {
        if !is_running.load(Ordering::SeqCst) {
            println!("🛑 Bot stopped. Exiting main loop.");
            break Ok(());
        }

        println!("\n🔄 Starting trading loop...");
        let mut feed = HyperliquidFeed::new(
            COIN.to_string(), 
            CANDLE_INTERVAL.to_string(), 
            is_live, 
            is_running.clone(),
            position_manager.clone(),
            telegram.clone()
        );

        // 1. SYNC STATE (Critical Security #1)
        if is_live {
            if let Some(trader) = &feed.trader {
                println!("🔄 Checking for existing positions on Hyperliquid...");
                match trader.get_open_positions().await {
                    Ok(positions) => {
                        let mut pm = position_manager.lock().await;
                        for (coin, size, entry_px, side) in positions {
                            if coin == COIN {
                                pm.sync_position(&coin, size, entry_px, &side);
                            }
                        }
                    },
                    Err(e) => eprintln!("⚠️ Failed to sync positions: {}", e),
                }
            }
        }
        
        if let Err(e) = feed.connect_and_trade().await {
            eprintln!("❌ Trading loop error: {}", e);
            
            // Notify Disconnection
            if let Some(bot) = &telegram {
                let _ = bot.send_default_message_with_menu_btn(&format!("⚠️ *Bot Disconnected* - Error: {}\nReconnecting in 5s...", e)).await;
            }

            eprintln!("⏳ Retrying in 5 seconds...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        } else {
            println!("⚠️ Connection closed cleanly. Reconnecting...");
            
            // Notify Clean Disconnection
            if let Some(bot) = &telegram {
                let _ = bot.send_default_message_with_menu_btn("⚠️ *Bot Disconnected* - Connection closed cleanly.\nReconnecting...").await;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
