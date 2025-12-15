// 💰 Position Manager - Gestion des positions avec bankroll dynamique
// Calcul automatique des tailles de position basé sur le risque (2% max loss)

use serde::{Deserialize, Serialize};

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
            risk_percentage: 2.0, // 2% max loss par trade
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
}

impl PositionManager {
    pub fn new(bankroll: f64) -> Self {
        Self {
            bankroll: BankrollInfo::new(bankroll),
            position: None,
            closed_trades: Vec::new(),
        }
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

        let position_size = self.bankroll.calculate_position_size(
            ((entry_price - stop_loss_price) / entry_price) * 100.0
        ) / entry_price;

        if position_size <= 0.0 || position_size * entry_price > self.bankroll.available_balance {
            eprintln!("❌ Position trop grande pour la bankroll disponible");
            return None;
        }

        let position = OpenPosition::new_long(entry_price, entry_time, position_size, stop_loss_price);
        self.bankroll.update_available_balance(
            self.bankroll.available_balance - position.position_value
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

        let position_size = self.bankroll.calculate_position_size(
            ((stop_loss_price - entry_price) / entry_price) * 100.0
        ) / entry_price;

        if position_size <= 0.0 || position_size * entry_price > self.bankroll.available_balance {
            eprintln!("❌ Position trop grande pour la bankroll disponible");
            return None;
        }

        let position = OpenPosition::new_short(entry_price, entry_time, position_size, stop_loss_price);
        self.bankroll.update_available_balance(
            self.bankroll.available_balance - position.position_value
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
