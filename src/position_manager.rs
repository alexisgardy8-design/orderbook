// 💰 Position Manager - Gestion des positions avec bankroll dynamique
// Calcul automatique des tailles de position basé sur le risque (2% max loss)

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Information sur la bankroll et les positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankrollInfo {
    /// Solde total en USDC
    pub total_balance: f64,
    /// Solde disponible (non en position)
    pub available_balance: f64,
    /// Pourcentage de risque par trade (défaut 2%)
    pub risk_percentage: f64,
}

impl BankrollInfo {
    pub fn new(total_balance: f64) -> Self {
        Self {
            total_balance,
            available_balance: total_balance,
            risk_percentage: 1.0, // 1% risk per trade (Target: 100% Position Size with 1% SL)
        }
    }

    /// Calcule la taille de position optimale basée sur le stop loss
    /// position_size = (bankroll * risk_pct) / (stop_loss_distance_pct)
    pub fn calculate_position_size(&self, stop_loss_pct: f64) -> f64 {
        if stop_loss_pct <= 0.0 {
            return 0.0;
        }
        
        let risk_amount = self.available_balance * (self.risk_percentage / 100.0);
        let position_value = risk_amount / (stop_loss_pct / 100.0);
        
        position_value
    }

    /// Met à jour le solde disponible
    pub fn update_available_balance(&mut self, new_balance: f64) {
        self.available_balance = new_balance;
    }
}

/// État d'une position active
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionState {
    None,
    Long,
    Short,
}

/// Information détaillée sur une position
#[derive(Debug, Clone)]
pub struct OpenPosition {
    pub state: PositionState,
    pub entry_price: f64,
    pub entry_time: u64,
    pub position_size: f64,           // Quantité de SOL
    pub position_value: f64,          // Valeur en USDC
    pub stop_loss_price: f64,         // Prix du stop loss
    pub stop_loss_pct: f64,           // Pourcentage du SL
    pub take_profit_price: Option<f64>, // TP optionnel
    pub unrealized_pnl: f64,          // P&L non réalisé
    pub unrealized_pnl_pct: f64,      // P&L % non réalisé
    pub entry_fee: f64,               // Frais d'entrée réels
    pub db_id: Option<i64>,           // ID Supabase
}

impl OpenPosition {
    /// Crée une nouvelle position LONG
    pub fn new_long(
        entry_price: f64,
        entry_time: u64,
        position_size: f64,
        stop_loss_price: f64,
    ) -> Self {
        let stop_loss_pct = ((entry_price - stop_loss_price) / entry_price) * 100.0;
        
        Self {
            state: PositionState::Long,
            entry_price,
            entry_time,
            position_size,
            position_value: position_size * entry_price,
            stop_loss_price,
            stop_loss_pct,
            take_profit_price: None,
            unrealized_pnl: 0.0,
            unrealized_pnl_pct: 0.0,
            entry_fee: 0.0,
            db_id: None,
        }
    }

    /// Crée une nouvelle position SHORT
    pub fn new_short(
        entry_price: f64,
        entry_time: u64,
        position_size: f64,
        stop_loss_price: f64,
    ) -> Self {
        let stop_loss_pct = ((stop_loss_price - entry_price) / entry_price) * 100.0;
        
        Self {
            state: PositionState::Short,
            entry_price,
            entry_time,
            position_size,
            position_value: position_size * entry_price,
            stop_loss_price,
            stop_loss_pct,
            take_profit_price: None,
            unrealized_pnl: 0.0,
            unrealized_pnl_pct: 0.0,
            entry_fee: 0.0,
            db_id: None,
        }
    }

    /// Met à jour le P&L non réalisé
    pub fn update_pnl(&mut self, current_price: f64) {
        match self.state {
            PositionState::Long => {
                let price_change = current_price - self.entry_price;
                self.unrealized_pnl = price_change * self.position_size;
                self.unrealized_pnl_pct = (price_change / self.entry_price) * 100.0;
            }
            PositionState::Short => {
                let price_change = self.entry_price - current_price;
                self.unrealized_pnl = price_change * self.position_size;
                self.unrealized_pnl_pct = (price_change / self.entry_price) * 100.0;
            }
            PositionState::None => {}
        }
    }

    /// Vérifie si le stop loss est atteint
    pub fn is_stop_loss_hit(&self, current_price: f64) -> bool {
        match self.state {
            PositionState::Long => current_price <= self.stop_loss_price,
            PositionState::Short => current_price >= self.stop_loss_price,
            PositionState::None => false,
        }
    }

    /// Vérifie si le take profit est atteint (si défini)
    pub fn is_take_profit_hit(&self, current_price: f64) -> bool {
        match self.take_profit_price {
            Some(tp) => match self.state {
                PositionState::Long => current_price >= tp,
                PositionState::Short => current_price <= tp,
                PositionState::None => false,
            },
            None => false,
        }
    }

    /// Ajoute un take profit
    pub fn set_take_profit(&mut self, tp_price: f64) {
        self.take_profit_price = Some(tp_price);
    }
}

/// Gestionnaire complet de position
pub struct PositionManager {
    pub bankroll: BankrollInfo,
    pub position: Option<OpenPosition>,
    pub closed_trades: Vec<ClosedTrade>,
    pub trade_timestamps: VecDeque<u64>, // Pour limiter la fréquence (Kill Switch)
    pub initial_balance_session: f64,    // Pour limiter le drawdown (Kill Switch)
    #[cfg(feature = "websocket")]
    pub supabase: Option<crate::supabase::SupabaseClient>,
    
    // Indicators for Telegram display
    pub last_adx: f64,
    pub last_regime: String,
    pub last_bollinger: Option<(f64, f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct ClosedTrade {
    pub entry_price: f64,
    pub exit_price: f64,
    pub position_size: f64,
    pub profit_loss: f64,
    pub profit_loss_pct: f64,
    pub entry_time: u64,
    pub exit_time: u64,
    pub trade_type: String, // "Long" or "Short"
    pub db_id: Option<i64>,
}

impl PositionManager {
    #[cfg(feature = "websocket")]
    pub fn new(bankroll: f64, supabase: Option<crate::supabase::SupabaseClient>) -> Self {
        Self {
            bankroll: BankrollInfo::new(bankroll),
            position: None,
            closed_trades: Vec::new(),
            trade_timestamps: VecDeque::new(),
            initial_balance_session: bankroll,
            supabase,
            last_adx: 0.0,
            last_regime: "Unknown".to_string(),
            last_bollinger: None,
        }
    }

    #[cfg(not(feature = "websocket"))]
    pub fn new(bankroll: f64) -> Self {
        Self {
            bankroll: BankrollInfo::new(bankroll),
            position: None,
            closed_trades: Vec::new(),
            trade_timestamps: VecDeque::new(),
            initial_balance_session: bankroll,
            last_adx: 0.0,
            last_regime: "Unknown".to_string(),
            last_bollinger: None,
        }
    }

    #[cfg(feature = "websocket")]
    pub async fn init(&mut self) {
        if let Some(client) = &self.supabase {
            println!("🔄 Syncing positions from Supabase...");
            if let Ok(positions) = client.fetch_open_positions().await {
                if let Some(db_pos) = positions.first() {
                    println!("✅ Found open position in DB: {} {}", db_pos.side, db_pos.coin);
                    
                    let state = if db_pos.side == "LONG" { PositionState::Long } else { PositionState::Short };
                    let stop_loss_pct = 1.0; // Default assumption if not stored
                    let stop_loss_price = if state == PositionState::Long {
                        db_pos.entry_price * 0.99
                    } else {
                        db_pos.entry_price * 1.01
                    };

                    self.position = Some(OpenPosition {
                        state,
                        entry_price: db_pos.entry_price,
                        entry_time: db_pos.created_at.map(|d| d.timestamp_millis() as u64).unwrap_or(0),
                        position_size: db_pos.size,
                        position_value: db_pos.size * db_pos.entry_price,
                        stop_loss_price,
                        stop_loss_pct,
                        take_profit_price: None,
                        unrealized_pnl: 0.0,
                        unrealized_pnl_pct: 0.0,
                        entry_fee: 0.0,
                        db_id: db_pos.id,
                    });
                } else {
                    println!("✅ No open positions in DB.");
                }
            } else {
                eprintln!("❌ Failed to fetch positions from Supabase");
            }
        }
    }

    /// Vérifie les limites de sécurité (Kill Switch)
    pub fn check_safety_limits(&mut self, max_drawdown_pct: f64, max_trades_per_hour: usize) -> Result<(), String> {
        // 1. Vérifier le Drawdown Max
        let current_equity = self.bankroll.total_balance;
        let drawdown_pct = ((self.initial_balance_session - current_equity) / self.initial_balance_session) * 100.0;
        
        if drawdown_pct > max_drawdown_pct {
            return Err(format!(
                "🚨 KILL SWITCH ACTIVATED: Max Drawdown Exceeded! Loss: {:.2}% (Limit: {:.2}%)",
                drawdown_pct, max_drawdown_pct
            ));
        }

        // 2. Vérifier la fréquence de trading (Max trades / heure)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        // Nettoyer les vieux timestamps (> 1h)
        while let Some(&t) = self.trade_timestamps.front() {
            if now - t > 3600 {
                self.trade_timestamps.pop_front();
            } else {
                break;
            }
        }

        if self.trade_timestamps.len() >= max_trades_per_hour {
            return Err(format!(
                "🚨 KILL SWITCH ACTIVATED: Trading Frequency Exceeded! {} trades in last hour (Limit: {})",
                self.trade_timestamps.len(), max_trades_per_hour
            ));
        }

        Ok(())
    }

    /// Synchronise une position existante depuis l'API (Recovery)
    pub fn sync_position(&mut self, coin: &str, amount: f64, entry_price: f64, side: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Estimer un SL large pour la sécurité (car on a perdu l'info originale)
        // On suppose 5% de SL par défaut pour la récupération
        let sl_price = if side == "Long" {
            entry_price * 0.95
        } else {
            entry_price * 1.05
        };

        let pos = if side == "Long" {
            OpenPosition::new_long(entry_price, now, amount.abs(), sl_price)
        } else {
            OpenPosition::new_short(entry_price, now, amount.abs(), sl_price)
        };

        println!("🔄 RECOVERY: Synced existing {} position on {}. Size: {}, Entry: ${}", 
            side, coin, amount, entry_price);
            
        self.position = Some(pos);
        
        // Mettre à jour le solde disponible (on considère que l'argent est engagé)
        // Note: C'est une approximation, l'API donnera le vrai solde dispo
    }

    /// Ouvre une position LONG avec gestion de risque
    pub fn open_long(
        &mut self,
        entry_price: f64,
        entry_time: u64,
        stop_loss_price: f64,
    ) -> Option<OpenPosition> {
        if self.position.is_some() {
            eprintln!("❌ Une position est déjà ouverte");
            return None;
        }

        let stop_loss_pct = ((entry_price - stop_loss_price) / entry_price) * 100.0;
        let mut position_value = self.bankroll.calculate_position_size(stop_loss_pct);

        // 1. Minimum Order Value ($10)
        if position_value < 10.0 {
            println!("⚠️ Position calculée (${:.2}) < $10. Ajustement à $10.", position_value);
            position_value = 10.0;
        }

        // 2. Max Leverage 1x (sauf si on a forcé à $10)
        if position_value > self.bankroll.available_balance {
            if position_value == 10.0 {
                println!("⚠️ Utilisation de levier pour atteindre le minimum de $10 (Balance: ${:.2})", self.bankroll.available_balance);
            } else {
                println!("⚠️ Position (${:.2}) > Balance. Limitation à 1x levier (${:.2})", position_value, self.bankroll.available_balance);
                position_value = self.bankroll.available_balance;
            }
        }

        let position_size = position_value / entry_price;

        let position = OpenPosition::new_long(entry_price, entry_time, position_size, stop_loss_price);
        
        // On ne déduit pas plus que ce qu'on a (si levier, on utilise tout le collatéral)
        let cost = f64::min(position.position_value, self.bankroll.available_balance);
        self.bankroll.update_available_balance(
            self.bankroll.available_balance - cost
        );
        
        self.position = Some(position.clone());
        Some(position)
    }

    /// Ouvre une position SHORT avec gestion de risque
    pub fn open_short(
        &mut self,
        entry_price: f64,
        entry_time: u64,
        stop_loss_price: f64,
    ) -> Option<OpenPosition> {
        if self.position.is_some() {
            eprintln!("❌ Une position est déjà ouverte");
            return None;
        }

        let stop_loss_pct = ((stop_loss_price - entry_price) / entry_price) * 100.0;
        let mut position_value = self.bankroll.calculate_position_size(stop_loss_pct);

        // 1. Minimum Order Value ($10)
        if position_value < 10.0 {
            println!("⚠️ Position calculée (${:.2}) < $10. Ajustement à $10.", position_value);
            position_value = 10.0;
        }

        // 2. Max Leverage 1x (sauf si on a forcé à $10)
        if position_value > self.bankroll.available_balance {
            if position_value == 10.0 {
                println!("⚠️ Utilisation de levier pour atteindre le minimum de $10 (Balance: ${:.2})", self.bankroll.available_balance);
            } else {
                println!("⚠️ Position (${:.2}) > Balance. Limitation à 1x levier (${:.2})", position_value, self.bankroll.available_balance);
                position_value = self.bankroll.available_balance;
            }
        }

        let position_size = position_value / entry_price;

        let position = OpenPosition::new_short(entry_price, entry_time, position_size, stop_loss_price);
        
        // On ne déduit pas plus que ce qu'on a (si levier, on utilise tout le collatéral)
        let cost = f64::min(position.position_value, self.bankroll.available_balance);
        self.bankroll.update_available_balance(
            self.bankroll.available_balance - cost
        );
        
        self.position = Some(position.clone());
        Some(position)
    }

    /// Ferme la position actuelle
    pub fn close_position(&mut self, exit_price: f64, exit_time: u64) -> Option<ClosedTrade> {
        if let Some(mut pos) = self.position.take() {
            pos.update_pnl(exit_price);

            let trade_type = match pos.state {
                PositionState::Long => "Long",
                PositionState::Short => "Short",
                PositionState::None => "Unknown",
            };

            let closed = ClosedTrade {
                entry_price: pos.entry_price,
                exit_price,
                position_size: pos.position_size,
                profit_loss: pos.unrealized_pnl,
                profit_loss_pct: pos.unrealized_pnl_pct,
                entry_time: pos.entry_time,
                exit_time,
                trade_type: trade_type.to_string(),
                db_id: pos.db_id,
            };

            // Restaurer la bankroll
            let exit_value = match pos.state {
                PositionState::Long => pos.position_size * exit_price,
                PositionState::Short => {
                    // Pour un short, on gagne si le prix baisse
                    let profit = pos.unrealized_pnl;
                    pos.position_value - profit
                }
                PositionState::None => 0.0,
            };

            self.bankroll.update_available_balance(
                self.bankroll.available_balance + exit_value
            );
            self.bankroll.total_balance += closed.profit_loss;

            self.closed_trades.push(closed.clone());
            
            // Enregistrer le timestamp pour le Kill Switch (fréquence)
            self.trade_timestamps.push_back(exit_time);
            
            Some(closed)
        } else {
            None
        }
    }

    /// Met à jour le P&L actuel
    pub fn update_current_pnl(&mut self, current_price: f64) {
        if let Some(pos) = &mut self.position {
            pos.update_pnl(current_price);
        }
    }

    /// Retourne les stats de trading
    pub fn get_stats(&self) -> TradingStats {
        let total_trades = self.closed_trades.len();
        let winning_trades = self.closed_trades.iter().filter(|t| t.profit_loss > 0.0).count();
        let losing_trades = total_trades - winning_trades;

        let total_profit: f64 = self.closed_trades.iter().map(|t| t.profit_loss).sum();
        let avg_profit = if total_trades > 0 {
            total_profit / total_trades as f64
        } else {
            0.0
        };

        let win_rate = if total_trades > 0 {
            (winning_trades as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };

        TradingStats {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate,
            total_profit,
            avg_profit,
            current_balance: self.bankroll.total_balance,
            initial_balance: self.bankroll.total_balance - total_profit,
            return_pct: if self.bankroll.total_balance > 0.0 {
                ((self.bankroll.total_balance - (self.bankroll.total_balance - total_profit)) 
                    / (self.bankroll.total_balance - total_profit)) * 100.0
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradingStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_profit: f64,
    pub avg_profit: f64,
    pub current_balance: f64,
    pub initial_balance: f64,
    pub return_pct: f64,
}
