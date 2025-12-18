// 🦎 Adaptive Strategy: Regime Switching (Bollinger + SuperTrend)
// Utilise ADX pour détecter le régime de marché et switcher entre stratégies

use std::collections::VecDeque;

/// Type de régime de marché détecté
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketRegime {
    Ranging,    // ADX < 25: Marché sans tendance (Bollinger)
    Trending,   // ADX >= 25: Marché directionnel (SuperTrend)
}

/// Type de position actuellement détenue
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionType {
    None,
    LongRange,      // Position prise en mode Range (Bollinger)
    LongTrend,      // Position prise en mode Trend haussier (SuperTrend)
    ShortTrend,     // Position prise en mode Trend baissier (SuperTrend short)
}

/// Signal de trading
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Signal {
    BuyRange,       // Achat en mode Range (Bollinger oversold)
    SellRange,      // Vente en mode Range (retour moyenne)
    BuyTrend,       // Achat en mode Trend haussier (breakout up)
    SellTrend,      // Vente en mode Trend haussier (trailing stop cassé)
    SellShort,      // Vente short en mode Trend baissier (breakout down)
    CoverShort,     // Rachat pour fermer short (trailing stop cassé)
    UpgradeToTrend, // Transformation: Range -> Trend
    Hold,
}

/// ADX (Average Directional Index) - Mesure la FORCE d'une tendance
/// Implémentation fidèle à TradingView (Wilder's Smoothing / RMA)
#[derive(Debug, Clone)]
pub struct ADX {
    period: usize,
    
    // Buffers pour l'initialisation (SMA)
    tr_buf: VecDeque<f64>,
    plus_dm_buf: VecDeque<f64>,
    minus_dm_buf: VecDeque<f64>,
    dx_buf: VecDeque<f64>,
    
    // Valeurs lissées (RMA)
    tr_smooth: f64,
    plus_dm_smooth: f64,
    minus_dm_smooth: f64,
    adx_smooth: f64,
    
    // État précédent
    prev_high: Option<f64>,
    prev_low: Option<f64>,
    prev_close: Option<f64>,
}

impl ADX {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            tr_buf: VecDeque::with_capacity(period),
            plus_dm_buf: VecDeque::with_capacity(period),
            minus_dm_buf: VecDeque::with_capacity(period),
            dx_buf: VecDeque::with_capacity(period),
            
            tr_smooth: 0.0,
            plus_dm_smooth: 0.0,
            minus_dm_smooth: 0.0,
            adx_smooth: 0.0,
            
            prev_high: None,
            prev_low: None,
            prev_close: None,
        }
    }

    pub fn update(&mut self, high: f64, low: f64, close: f64) -> Option<f64> {
        if let (Some(ph), Some(pl), Some(pc)) = (self.prev_high, self.prev_low, self.prev_close) {
            // 1. True Range (TR)
            let tr1 = high - low;
            let tr2 = (high - pc).abs();
            let tr3 = (low - pc).abs();
            let tr = tr1.max(tr2).max(tr3);

            // 2. Directional Movement
            let up_move = high - ph;
            let down_move = pl - low;

            let plus_dm = if up_move > down_move && up_move > 0.0 { up_move } else { 0.0 };
            let minus_dm = if down_move > up_move && down_move > 0.0 { down_move } else { 0.0 };

            // --- Phase 1: Initialisation TR/DM (SMA) ---
            if self.tr_buf.len() < self.period {
                self.tr_buf.push_back(tr);
                self.plus_dm_buf.push_back(plus_dm);
                self.minus_dm_buf.push_back(minus_dm);
                
                // Si on vient de remplir le buffer, on calcule la première SMA
                if self.tr_buf.len() == self.period {
                    self.tr_smooth = self.tr_buf.iter().sum::<f64>() / self.period as f64;
                    self.plus_dm_smooth = self.plus_dm_buf.iter().sum::<f64>() / self.period as f64;
                    self.minus_dm_smooth = self.minus_dm_buf.iter().sum::<f64>() / self.period as f64;
                }
            } else {
                // --- Phase 2: Lissage Wilder (RMA) ---
                // RMA = (Prev * (N-1) + Curr) / N
                let alpha = 1.0 / self.period as f64;
                self.tr_smooth = (self.tr_smooth * (1.0 - alpha)) + (tr * alpha);
                self.plus_dm_smooth = (self.plus_dm_smooth * (1.0 - alpha)) + (plus_dm * alpha);
                self.minus_dm_smooth = (self.minus_dm_smooth * (1.0 - alpha)) + (minus_dm * alpha);
            }

            // Calcul du DX (si on a commencé à lisser)
            if self.tr_buf.len() >= self.period {
                let mut dx = 0.0;
                if self.tr_smooth != 0.0 {
                    let di_plus = 100.0 * (self.plus_dm_smooth / self.tr_smooth);
                    let di_minus = 100.0 * (self.minus_dm_smooth / self.tr_smooth);
                    let sum_di = di_plus + di_minus;
                    if sum_di != 0.0 {
                        dx = 100.0 * (di_plus - di_minus).abs() / sum_di;
                    }
                }

                // --- Phase 3: Initialisation ADX (SMA du DX) ---
                if self.dx_buf.len() < self.period {
                    self.dx_buf.push_back(dx);
                    
                    if self.dx_buf.len() == self.period {
                        self.adx_smooth = self.dx_buf.iter().sum::<f64>() / self.period as f64;
                        // On a enfin une valeur ADX valide !
                    }
                } else {
                    // --- Phase 4: Lissage ADX (RMA du DX) ---
                    let alpha = 1.0 / self.period as f64;
                    self.adx_smooth = (self.adx_smooth * (1.0 - alpha)) + (dx * alpha);
                }
            }
        }

        self.prev_high = Some(high);
        self.prev_low = Some(low);
        self.prev_close = Some(close);

        // On ne retourne une valeur que si tout est initialisé (2 * period)
        if self.dx_buf.len() >= self.period {
            Some(self.adx_smooth)
        } else {
            None
        }
    }
}

/// SuperTrend - Trailing Stop dynamique basé sur ATR
#[derive(Debug, Clone)]
pub struct SuperTrend {
    period: usize,
    multiplier: f64,
    atr_values: VecDeque<f64>,
    prev_close: Option<f64>,
    supertrend_value: f64,
    is_uptrend: bool,
}

impl SuperTrend {
    pub fn new(period: usize, multiplier: f64) -> Self {
        Self {
            period,
            multiplier,
            atr_values: VecDeque::new(),
            prev_close: None,
            supertrend_value: 0.0,
            is_uptrend: true,
        }
    }

    pub fn update(&mut self, high: f64, low: f64, close: f64) -> Option<(f64, bool)> {
        // 1. Calculate TR
        let tr = if let Some(pc) = self.prev_close {
            let hl = high - low;
            let hc = (high - pc).abs();
            let lc = (low - pc).abs();
            hl.max(hc).max(lc)
        } else {
            high - low
        };

        self.atr_values.push_back(tr);
        if self.atr_values.len() > self.period {
            self.atr_values.pop_front();
        }

        self.prev_close = Some(close);

        if self.atr_values.len() < self.period {
            return None;
        }

        // 2. Calculate ATR
        let atr: f64 = self.atr_values.iter().sum::<f64>() / self.period as f64;

        // 3. Calculate Basic Bands
        let hl_avg = (high + low) / 2.0;
        let basic_upper = hl_avg + (self.multiplier * atr);
        let basic_lower = hl_avg - (self.multiplier * atr);

        // 4. Determine SuperTrend
        let new_supertrend = if self.is_uptrend {
            let lower_band = basic_lower.max(self.supertrend_value);
            if close <= lower_band {
                self.is_uptrend = false;
                basic_upper
            } else {
                lower_band
            }
        } else {
            let upper_band = basic_upper.min(self.supertrend_value);
            if close >= upper_band {
                self.is_uptrend = true;
                basic_lower
            } else {
                upper_band
            }
        };

        self.supertrend_value = new_supertrend;
        Some((self.supertrend_value, self.is_uptrend))
    }

    pub fn get_trend(&self) -> bool {
        self.is_uptrend
    }
}

/// Bollinger Bands (réutilisé de bollinger_strategy.rs)
#[derive(Debug, Clone)]
pub struct BollingerBands {
    period: usize,
    std_dev_multiplier: f64,
    prices: VecDeque<f64>,
}

impl BollingerBands {
    pub fn new(period: usize, std_dev_multiplier: f64) -> Self {
        Self {
            period,
            std_dev_multiplier,
            prices: VecDeque::with_capacity(period + 1),
        }
    }

    pub fn update(&mut self, price: f64) -> Option<(f64, f64, f64)> {
        self.prices.push_back(price);
        
        if self.prices.len() > self.period {
            self.prices.pop_front();
        }

        if self.prices.len() < self.period {
            return None;
        }

        let sum: f64 = self.prices.iter().sum();
        let mean = sum / self.period as f64;

        let variance: f64 = self.prices
            .iter()
            .map(|value| {
                let diff = mean - *value;
                diff * diff
            })
            .sum::<f64>()
            / self.period as f64;

        let std_dev = variance.sqrt();

        Some((
            mean - self.std_dev_multiplier * std_dev,
            mean,
            mean + self.std_dev_multiplier * std_dev,
        ))
    }
}

/// RSI (Relative Strength Index)
#[derive(Debug, Clone)]
pub struct RSI {
    period: usize,
    prices: VecDeque<f64>,
    prev_avg_gain: Option<f64>,
    prev_avg_loss: Option<f64>,
}

impl RSI {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            prices: VecDeque::new(),
            prev_avg_gain: None,
            prev_avg_loss: None,
        }
    }

    pub fn update(&mut self, price: f64) -> Option<f64> {
        self.prices.push_back(price);
        if self.prices.len() > self.period + 1 {
            self.prices.pop_front();
        }

        if self.prices.len() < 2 {
            return None;
        }

        // Initial calculation (first time we have enough data)
        if self.prev_avg_gain.is_none() {
            if self.prices.len() <= self.period {
                return None;
            }

            let mut gains = 0.0;
            let mut losses = 0.0;

            for i in 1..self.prices.len() {
                let change = self.prices[i] - self.prices[i - 1];
                if change > 0.0 {
                    gains += change;
                } else {
                    losses += change.abs();
                }
            }

            let avg_gain = gains / self.period as f64;
            let avg_loss = losses / self.period as f64;

            self.prev_avg_gain = Some(avg_gain);
            self.prev_avg_loss = Some(avg_loss);

            let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
            return Some(100.0 - (100.0 / (1.0 + rs)));
        }

        // Smoothed calculation
        let current_price = *self.prices.back().unwrap();
        let prev_price = self.prices[self.prices.len() - 2];
        let change = current_price - prev_price;

        let current_gain = if change > 0.0 { change } else { 0.0 };
        let current_loss = if change < 0.0 { change.abs() } else { 0.0 };

        let prev_avg_gain = self.prev_avg_gain.unwrap();
        let prev_avg_loss = self.prev_avg_loss.unwrap();

        let avg_gain = ((prev_avg_gain * (self.period as f64 - 1.0)) + current_gain) / self.period as f64;
        let avg_loss = ((prev_avg_loss * (self.period as f64 - 1.0)) + current_loss) / self.period as f64;

        self.prev_avg_gain = Some(avg_gain);
        self.prev_avg_loss = Some(avg_loss);

        let rs = if avg_loss == 0.0 { 100.0 } else { avg_gain / avg_loss };
        Some(100.0 - (100.0 / (1.0 + rs)))
    }
}

/// Configuration de la stratégie adaptative
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    // Bollinger (Range)
    pub bb_period: usize,
    pub bb_std_dev: f64,
    pub rsi_period: usize,
    pub rsi_oversold: f64,     // Pas utilisé dans cette version simplifiée
    
    // SuperTrend (Trend)
    pub st_period: usize,
    pub st_multiplier: f64,
    
    // ADX (Regime Detection)
    pub adx_period: usize,
    pub adx_threshold: f64,    // < threshold = Range, >= threshold = Trend

    // Safety / Kill Switch
    pub max_daily_drawdown_pct: f64, // e.g. 5.0 for 5%
    pub max_trades_per_hour: usize,  // e.g. 5
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            bb_period: 20,
            bb_std_dev: 2.0,
            rsi_period: 14,
            rsi_oversold: 30.0,
            st_period: 10,
            st_multiplier: 3.0,
            adx_period: 14,
            adx_threshold: 20.0,
            max_daily_drawdown_pct: 100.0, // Désactivé (100% autorisé)
            max_trades_per_hour: 5,        // Stop if > 5 trades/hour (algo going crazy)
        }
    }
}


/// Stratégie Adaptative complète
pub struct AdaptiveStrategy {
    config: AdaptiveConfig,
    bollinger: BollingerBands,
    supertrend: SuperTrend,
    adx: ADX,
    rsi: RSI,
    current_regime: MarketRegime,
    position_type: PositionType,
    entry_price: Option<f64>,
    last_adx: f64,
    last_bollinger: Option<(f64, f64, f64)>,
    last_rsi: f64,
}

impl AdaptiveStrategy {
    pub fn new(config: AdaptiveConfig) -> Self {
        let bollinger = BollingerBands::new(config.bb_period, config.bb_std_dev);
        let supertrend = SuperTrend::new(config.st_period, config.st_multiplier);
        let adx = ADX::new(config.adx_period);
        let rsi = RSI::new(config.rsi_period);

        Self {
            config,
            bollinger,
            supertrend,
            adx,
            rsi,
            current_regime: MarketRegime::Ranging,
            position_type: PositionType::None,
            entry_price: None,
            last_adx: 0.0,
            last_bollinger: None,
            last_rsi: 50.0,
        }
    }

    /// Vérifie si une condition de sortie est remplie en cours de bougie (Intra-Candle)
    /// Ne modifie pas l'état des indicateurs, mais peut retourner un Signal de sortie.
    pub fn check_exit_condition(&self, current_high: f64, _current_low: f64, _current_close: f64) -> Option<Signal> {
        match self.position_type {
            PositionType::LongRange => {
                if let Some((_, middle_band, _)) = self.last_bollinger {
                    // Si le prix touche la moyenne mobile (Middle Band)
                    if current_high >= middle_band {
                        return Some(Signal::SellRange);
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Force la sortie de position (utilisé après un signal intra-candle)
    pub fn force_exit(&mut self) {
        self.position_type = PositionType::None;
        self.entry_price = None;
    }

    /// Met à jour la stratégie avec OHLC et retourne un signal
    pub fn update(&mut self, high: f64, low: f64, close: f64) -> Signal {
        // 1. Mise à jour des indicateurs
        let bb_result = self.bollinger.update(close);
        let st_result = self.supertrend.update(high, low, close);
        let adx_result = self.adx.update(high, low, close);
        let rsi_result = self.rsi.update(close);

        let (lower_band, middle_band, upper_band) = match bb_result {
            Some(bands) => {
                self.last_bollinger = Some(bands);
                bands
            },
            None => return Signal::Hold,
        };

        let (_st_value, st_uptrend) = match st_result {
            Some(st) => st,
            None => return Signal::Hold,
        };

        let adx_value = match adx_result {
            Some(adx) => {
                self.last_adx = adx;
                adx
            },
            None => return Signal::Hold,
        };

        if let Some(rsi) = rsi_result {
            self.last_rsi = rsi;
        }

        // 2. Détection du régime de marché
        let new_regime = if adx_value >= self.config.adx_threshold {
            MarketRegime::Trending
        } else {
            MarketRegime::Ranging
        };
        self.current_regime = new_regime;

        // 3. Logique de trading selon le régime (BIDIRECTIONAL: LONG + SHORT)
        match self.position_type {
            PositionType::None => {
                match self.current_regime {
                    MarketRegime::Ranging => {
                        // BOLLINGER: Achat sur oversold (long only en range)
                        if close < lower_band {
                            self.position_type = PositionType::LongRange;
                            self.entry_price = Some(close);
                            return Signal::BuyRange;
                        }
                    }
                    MarketRegime::Trending => {
                        // SUPERTREND: Détection de la direction de la tendance
                        if st_uptrend && close > upper_band {
                            // TENDANCE HAUSSIÈRE: Achat long
                            self.position_type = PositionType::LongTrend;
                            self.entry_price = Some(close);
                            return Signal::BuyTrend;
                        } else if !st_uptrend && close < lower_band {
                            // TENDANCE BAISSIÈRE: Vente short
                            self.position_type = PositionType::ShortTrend;
                            self.entry_price = Some(close);
                            return Signal::SellShort;
                        }
                    }
                }
            }
            PositionType::LongRange => {
                // En position Range: sortie rapide au milieu
                // CHECK HIGH instead of CLOSE to capture touch
                if high >= middle_band {
                    self.position_type = PositionType::None;
                    self.entry_price = None;
                    return Signal::SellRange;
                }
                
                // UPGRADE: Si le marché passe en Trend et qu'on est profitable
                // On transforme la position Range en position Trend
                if self.current_regime == MarketRegime::Trending && close > middle_band {
                    self.position_type = PositionType::LongTrend;
                    return Signal::UpgradeToTrend;
                }
                
                // Stop Loss si casse forte
                if close < lower_band * 0.95 {
                    self.position_type = PositionType::None;
                    self.entry_price = None;
                    return Signal::SellRange;
                }
            }
            PositionType::LongTrend => {
                // En position Trend haussier: on ne sort que si SuperTrend casse
                if !st_uptrend {
                    self.position_type = PositionType::None;
                    self.entry_price = None;
                    return Signal::SellTrend;
                }
            }
            PositionType::ShortTrend => {
                // En position Trend baissier (SHORT): on ne sort que si SuperTrend remonte
                if st_uptrend {
                    self.position_type = PositionType::None;
                    self.entry_price = None;
                    return Signal::CoverShort;
                }
            }
        }

        Signal::Hold
    }

    pub fn get_regime(&self) -> MarketRegime {
        self.current_regime
    }

    pub fn get_position_type(&self) -> PositionType {
        self.position_type
    }

    pub fn reset(&mut self) {
        self.bollinger = BollingerBands::new(self.config.bb_period, self.config.bb_std_dev);
        self.supertrend = SuperTrend::new(self.config.st_period, self.config.st_multiplier);
        self.adx = ADX::new(self.config.adx_period);
        self.current_regime = MarketRegime::Ranging;
        self.position_type = PositionType::None;
        self.entry_price = None;
    }

    /// Getters pour l'affichage en live
    pub fn get_current_regime(&self) -> MarketRegime {
        self.current_regime
    }

    pub fn get_supertrend_status(&self) -> bool {
        self.supertrend.get_trend()
    }

    pub fn get_adx_value(&self) -> f64 {
        self.last_adx
    }

    pub fn get_bollinger_bands(&self) -> Option<(f64, f64, f64)> {
        self.last_bollinger
    }

    pub fn get_rsi_value(&self) -> f64 {
        self.last_rsi
    }

    /// Force set the position type (Used for Manual Sync)
    pub fn force_position_type(&mut self, pos_type: PositionType) {
        self.position_type = pos_type;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adx_calculation() {
        let mut adx = ADX::new(14);
        
        // Warmup (Need 2 * period = 28 candles for full initialization)
        for i in 0..30 {
            let price = 100.0 + (i as f64);
            adx.update(price + 1.0, price - 1.0, price);
        }
        
        let result = adx.update(131.0, 129.0, 130.0);
        assert!(result.is_some());
        assert!(result.unwrap() >= 0.0 && result.unwrap() <= 100.0);
    }

    #[test]
    fn test_supertrend() {
        let mut st = SuperTrend::new(10, 3.0);
        
        for i in 0..15 {
            let price = 100.0 + (i as f64);
            st.update(price + 0.5, price - 0.5, price);
        }
        
        let result = st.update(115.5, 114.5, 115.0);
        assert!(result.is_some());
    }
}
