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
#[derive(Debug, Clone)]
pub struct ADX {
    period: usize,
    tr_values: VecDeque<f64>,
    plus_dm_values: VecDeque<f64>,
    minus_dm_values: VecDeque<f64>,
    prev_high: Option<f64>,
    prev_low: Option<f64>,
    prev_close: Option<f64>,
}

impl ADX {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            tr_values: VecDeque::new(),
            plus_dm_values: VecDeque::new(),
            minus_dm_values: VecDeque::new(),
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

            self.tr_values.push_back(tr);
            self.plus_dm_values.push_back(plus_dm);
            self.minus_dm_values.push_back(minus_dm);

            if self.tr_values.len() > self.period {
                self.tr_values.pop_front();
                self.plus_dm_values.pop_front();
                self.minus_dm_values.pop_front();
            }
        }

        self.prev_high = Some(high);
        self.prev_low = Some(low);
        self.prev_close = Some(close);

        if self.tr_values.len() < self.period {
            return None;
        }

        // 3. Smoothed TR et DM
        let tr_sum: f64 = self.tr_values.iter().sum();
        let plus_dm_sum: f64 = self.plus_dm_values.iter().sum();
        let minus_dm_sum: f64 = self.minus_dm_values.iter().sum();

        // 4. Directional Indicators
        let di_plus = 100.0 * (plus_dm_sum / tr_sum);
        let di_minus = 100.0 * (minus_dm_sum / tr_sum);

        // 5. DX (Directional Index)
        let dx_sum = di_plus + di_minus;
        if dx_sum == 0.0 {
            return Some(0.0);
        }
        
        let dx = 100.0 * (di_plus - di_minus).abs() / dx_sum;
        
        // 6. ADX = Average of DX (simplified: instantaneous DX)
        // Note: True Wilder's ADX uses exponential smoothing of DX
        Some(dx)
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

/// Configuration de la stratégie adaptative
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    // Bollinger (Range)
    pub bb_period: usize,
    pub bb_std_dev: f64,
    pub rsi_oversold: f64,     // Pas utilisé dans cette version simplifiée
    
    // SuperTrend (Trend)
    pub st_period: usize,
    pub st_multiplier: f64,
    
    // ADX (Regime Detection)
    pub adx_period: usize,
    pub adx_threshold: f64,    // < threshold = Range, >= threshold = Trend
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            bb_period: 20,
            bb_std_dev: 2.0,
            rsi_oversold: 30.0,
            st_period: 10,
            st_multiplier: 3.0,
            adx_period: 14,
            adx_threshold: 25.0,
        }
    }
}

/// Stratégie Adaptative complète
pub struct AdaptiveStrategy {
    config: AdaptiveConfig,
    bollinger: BollingerBands,
    supertrend: SuperTrend,
    adx: ADX,
    current_regime: MarketRegime,
    position_type: PositionType,
    entry_price: Option<f64>,
}

impl AdaptiveStrategy {
    pub fn new(config: AdaptiveConfig) -> Self {
        let bollinger = BollingerBands::new(config.bb_period, config.bb_std_dev);
        let supertrend = SuperTrend::new(config.st_period, config.st_multiplier);
        let adx = ADX::new(config.adx_period);

        Self {
            config,
            bollinger,
            supertrend,
            adx,
            current_regime: MarketRegime::Ranging,
            position_type: PositionType::None,
            entry_price: None,
        }
    }

    /// Met à jour la stratégie avec OHLC et retourne un signal
    pub fn update(&mut self, high: f64, low: f64, close: f64) -> Signal {
        // 1. Mise à jour des indicateurs
        let bb_result = self.bollinger.update(close);
        let st_result = self.supertrend.update(high, low, close);
        let adx_result = self.adx.update(high, low, close);

        let (lower_band, middle_band, upper_band) = match bb_result {
            Some(bands) => bands,
            None => return Signal::Hold,
        };

        let (_st_value, st_uptrend) = match st_result {
            Some(st) => st,
            None => return Signal::Hold,
        };

        let adx_value = match adx_result {
            Some(adx) => adx,
            None => return Signal::Hold,
        };

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
                if close >= middle_band {
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

    pub fn get_adx_value(&self) -> f64 {
        // Calculer la vraie valeur d'ADX si possible
        // Pour l'instant, approximation basée sur la longueur du buffer
        if self.adx.tr_values.len() >= self.config.adx_period {
            // On pourrait calculer la vraie valeur ici
            25.0 // Valeur placeholder
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adx_calculation() {
        let mut adx = ADX::new(14);
        
        // Warmup
        for i in 0..20 {
            let price = 100.0 + (i as f64);
            adx.update(price + 1.0, price - 1.0, price);
        }
        
        let result = adx.update(120.0, 118.0, 119.0);
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
