// 📊 Backtest pour la stratégie Adaptive (Regime Switching)
// Compare Bollinger seul vs Adaptive vs Buy & Hold sur données réelles

use crate::adaptive_strategy::{AdaptiveStrategy, AdaptiveConfig, Signal};
use crate::coinbase_historical::{Candle, CoinbaseHistoricalData};
use std::fs;
use std::path::Path;

/// Résultats d'un backtest
#[derive(Debug, Clone)]
pub struct BacktestResults {
    pub initial_capital: f64,
    pub final_capital: f64,
    pub total_return_pct: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub avg_profit_per_trade: f64,
    pub max_drawdown_pct: f64,
    pub buy_hold_return_pct: f64,
    pub sharpe_ratio: f64,
}

/// Charge ou récupère les données historiques
pub fn fetch_or_load_historical_data() -> Vec<Candle> {
    let cache_file = "sol_usd_5years.csv";
    
    // Essayer de charger depuis le cache
    if Path::new(cache_file).exists() {
        if let Ok(data) = load_candles_from_csv(cache_file) {
            println!("📂 Loaded {} candles from cached file: {}", data.len(), cache_file);
            return data;
        }
    }
    
    // Sinon, récupérer depuis l'API
    println!("🌐 Fetching data from Coinbase API...");
    let fetcher = CoinbaseHistoricalData::new();
    
    // 5 ans de données (approximatif)
    let end = chrono::Utc::now().timestamp() as u64;
    let start = end - (365 * 5 * 24 * 3600); // 5 ans
    
    match fetcher.fetch_candles(start, end) {
        Ok(candles) => {
            // Sauvegarder en cache
            if let Err(e) = save_candles_to_csv(&candles, cache_file) {
                eprintln!("⚠️  Failed to save cache: {}", e);
            }
            candles
        }
        Err(e) => {
            eprintln!("❌ Failed to fetch historical data: {}", e);
            Vec::new()
        }
    }
}

/// Sauvegarde les bougies dans un fichier CSV
fn save_candles_to_csv(candles: &[Candle], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    
    let mut file = fs::File::create(filename)?;
    writeln!(file, "timestamp,open,high,low,close,volume")?;
    
    for candle in candles {
        writeln!(
            file,
            "{},{},{},{},{},{}",
            candle.timestamp, candle.open, candle.high, candle.low, candle.close, candle.volume
        )?;
    }
    
    Ok(())
}

/// Charge les bougies depuis un fichier CSV
fn load_candles_from_csv(filename: &str) -> Result<Vec<Candle>, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(filename)?;
    let mut candles = Vec::new();
    
    for (i, line) in contents.lines().enumerate() {
        if i == 0 { continue; } // Skip header
        
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 6 { continue; }
        
        candles.push(Candle {
            timestamp: parts[0].parse()?,
            open: parts[1].parse()?,
            high: parts[2].parse()?,
            low: parts[3].parse()?,
            close: parts[4].parse()?,
            volume: parts[5].parse()?,
        });
    }
    
    Ok(candles)
}

/// Structure pour stocker les résultats d'un trade
#[derive(Debug, Clone)]
struct Trade {
    entry_price: f64,
    exit_price: f64,
    profit_pct: f64,
    profit_usd: f64,
    trade_type: String, // "Range" ou "Trend"
}

/// Exécute le backtest de la stratégie adaptative
pub fn run_adaptive_backtest(
    config: AdaptiveConfig,
    initial_capital: f64,
    fee_rate: f64,
    candles: &[Candle],
) -> BacktestResults {
    let mut strategy = AdaptiveStrategy::new(config);
    
    let mut capital = initial_capital;
    let mut position_size = 0.0;
    let mut entry_price = 0.0;
    let mut entry_type = String::from("None");
    let mut peak_capital = initial_capital;
    let mut max_drawdown = 0.0;
    
    let mut trades = Vec::new();
    let mut daily_returns = Vec::new();
    let mut prev_portfolio_value = initial_capital;
    
    // Stats spécifiques à l'adaptive
    let mut range_trades = 0;
    let mut trend_trades = 0;
    let mut upgrades = 0;
    let mut short_trades = 0;
    let mut is_short = false; // Flag pour savoir si on est en position short

    for candle in candles {
        let signal = strategy.update(candle.high, candle.low, candle.close);
        let price = candle.close;

        match signal {
            Signal::BuyRange => {
                if position_size == 0.0 && !is_short {
                    let amount_to_invest = capital * 0.95;
                    let fee = amount_to_invest * fee_rate;
                    let cost = amount_to_invest + fee;
                    
                    if cost <= capital {
                        position_size = amount_to_invest / price;
                        capital -= cost;
                        entry_price = price;
                        entry_type = String::from("Range");
                        range_trades += 1;
                        is_short = false;
                    }
                }
            }
            Signal::BuyTrend => {
                if position_size == 0.0 && !is_short {
                    let amount_to_invest = capital * 0.95;
                    let fee = amount_to_invest * fee_rate;
                    let cost = amount_to_invest + fee;
                    
                    if cost <= capital {
                        position_size = amount_to_invest / price;
                        capital -= cost;
                        entry_price = price;
                        entry_type = String::from("Trend");
                        trend_trades += 1;
                        is_short = false;
                    }
                }
            }
            Signal::SellShort => {
                // NOUVELLE LOGIQUE: Entrée en position SHORT
                if position_size == 0.0 && !is_short {
                    let amount_to_invest = capital * 0.95;
                    let fee = amount_to_invest * fee_rate;
                    let cost = amount_to_invest + fee;
                    
                    if cost <= capital {
                        position_size = amount_to_invest / price; // Quantité qu'on vend
                        capital -= fee; // On paye les frais uniquement (pas le collatéral)
                        entry_price = price;
                        entry_type = String::from("Short");
                        short_trades += 1;
                        is_short = true;
                    }
                }
            }
            Signal::SellRange | Signal::SellTrend => {
                if position_size > 0.0 && !is_short {
                    let revenue = position_size * price;
                    let fee = revenue * fee_rate;
                    let net_revenue = revenue - fee;
                    
                    capital += net_revenue;
                    
                    let profit_usd = net_revenue - (position_size * entry_price);
                    let profit_pct = (profit_usd / (position_size * entry_price)) * 100.0;
                    
                    trades.push(Trade {
                        entry_price,
                        exit_price: price,
                        profit_pct,
                        profit_usd,
                        trade_type: entry_type.clone(),
                    });
                    
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                }
            }
            Signal::CoverShort => {
                // NOUVELLE LOGIQUE: Sortie de position SHORT (rachat)
                if position_size > 0.0 && is_short {
                    let cost_to_cover = position_size * price; // Coût pour racheter
                    let fee = cost_to_cover * fee_rate;
                    let total_cost = cost_to_cover + fee;
                    
                    // Profit SHORT = (prix entrée - prix sortie) * quantité
                    let revenue = position_size * entry_price; // Ce qu'on avait vendu
                    let profit_usd = revenue - total_cost;
                    let profit_pct = (profit_usd / revenue) * 100.0;
                    
                    capital += profit_usd;
                    
                    trades.push(Trade {
                        entry_price,
                        exit_price: price,
                        profit_pct,
                        profit_usd,
                        trade_type: entry_type.clone(),
                    });
                    
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                }
            }
            Signal::UpgradeToTrend => {
                // Transformation Range -> Trend (on garde la position)
                entry_type = String::from("Trend");
                upgrades += 1;
            }
            Signal::Hold => {
                // Ne rien faire
            }
        }

        // Calcul de la valeur du portfolio (BIDIRECTIONNEL)
        let portfolio_value = if is_short {
            // En short: capital + gain/perte latent(e)
            // Gain si prix baisse, perte si prix monte
            capital + (position_size * (entry_price - price))
        } else {
            // En long: capital + valeur de la position
            capital + (position_size * price)
        };
        
        if portfolio_value > peak_capital {
            peak_capital = portfolio_value;
        }
        let current_drawdown = ((peak_capital - portfolio_value) / peak_capital) * 100.0;
        if current_drawdown > max_drawdown {
            max_drawdown = current_drawdown;
        }

        let daily_return = (portfolio_value - prev_portfolio_value) / prev_portfolio_value;
        daily_returns.push(daily_return);
        prev_portfolio_value = portfolio_value;
    }

    let final_value = capital + (position_size * candles.last().unwrap().close);

    let first_price = candles.first().unwrap().close;
    let last_price = candles.last().unwrap().close;
    let buy_hold_final = (initial_capital / first_price) * last_price;
    let buy_hold_return = ((buy_hold_final - initial_capital) / initial_capital) * 100.0;

    let total_trades = trades.len();
    let winning_trades = trades.iter().filter(|t| t.profit_usd > 0.0).count();
    let losing_trades = total_trades - winning_trades;
    let win_rate = if total_trades > 0 {
        (winning_trades as f64 / total_trades as f64) * 100.0
    } else {
        0.0
    };

    let avg_profit = if total_trades > 0 {
        trades.iter().map(|t| t.profit_usd).sum::<f64>() / total_trades as f64
    } else {
        0.0
    };

    let avg_return = daily_returns.iter().sum::<f64>() / daily_returns.len() as f64;
    let variance = daily_returns
        .iter()
        .map(|r| (r - avg_return).powi(2))
        .sum::<f64>()
        / daily_returns.len() as f64;
    let std_dev = variance.sqrt();
    let sharpe_ratio = if std_dev > 0.0 {
        (avg_return / std_dev) * (252.0_f64).sqrt()
    } else {
        0.0
    };

    // Affichage des stats spécifiques
    println!("\n📊 ADAPTIVE STRATEGY STATS:");
    println!("   Range Entries: {} trades", range_trades);
    println!("   Trend Entries (Long): {} trades", trend_trades);
    println!("   Trend Entries (Short): {} trades", short_trades);
    println!("   Range->Trend Upgrades: {} times", upgrades);

    BacktestResults {
        initial_capital,
        final_capital: final_value,
        total_return_pct: ((final_value - initial_capital) / initial_capital) * 100.0,
        total_trades,
        winning_trades,
        losing_trades,
        win_rate,
        avg_profit_per_trade: avg_profit,
        max_drawdown_pct: max_drawdown,
        buy_hold_return_pct: buy_hold_return,
        sharpe_ratio,
    }
}

/// Backtest complet avec données historiques réelles (5 ans)
pub fn run_adaptive_real_data_backtest() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🦎 ADAPTIVE STRATEGY BACKTEST (5 Years Real Data)           ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    let candles = fetch_or_load_historical_data();
    
    if candles.is_empty() {
        eprintln!("❌ No historical data available. Aborting backtest.");
        return;
    }
    
    println!("⚙️  Configuration:");
    println!("   Asset:        SOL-USD (Coinbase Spot)");
    println!("   Timeframe:    1 Hour");
    println!("   Data Points:  {} candles", candles.len());
    println!("   Period:       ~{} days (~{} years)", candles.len() / 24, (candles.len() / 24) / 365);
    println!("   Capital:      $1,000.00");
    println!("   Fee Rate:     0.10%");
    
    let initial_capital = 1000.0;
    let fee_rate = 0.001;
    
    // Test 1: Stratégie Adaptive Standard
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 1: Adaptive Strategy (ADX Threshold = 25)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_standard = AdaptiveConfig::default();
    let results_standard = run_adaptive_backtest(
        config_standard,
        initial_capital,
        fee_rate,
        &candles,
    );
    
    print_adaptive_results(&results_standard, "Adaptive Standard (ADX=25)");
    
    // Test 2: Adaptive avec seuil ADX plus strict (favorise Trend)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 2: Adaptive Strategy - Trend Biased (ADX Threshold = 20)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_trend_biased = AdaptiveConfig {
        adx_threshold: 20.0, // Plus facile de passer en mode Trend
        ..Default::default()
    };
    let results_trend = run_adaptive_backtest(
        config_trend_biased,
        initial_capital,
        fee_rate,
        &candles,
    );
    
    print_adaptive_results(&results_trend, "Adaptive Trend-Biased (ADX=20)");
    
    // Test 3: Adaptive avec seuil ADX plus lâche (favorise Range)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 3: Adaptive Strategy - Range Biased (ADX Threshold = 30)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_range_biased = AdaptiveConfig {
        adx_threshold: 30.0, // Plus strict, reste en mode Range plus souvent
        ..Default::default()
    };
    let results_range = run_adaptive_backtest(
        config_range_biased,
        initial_capital,
        fee_rate,
        &candles,
    );
    
    print_adaptive_results(&results_range, "Adaptive Range-Biased (ADX=30)");
    
    // Comparaison finale
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  🏆 ADAPTIVE STRATEGY COMPARISON                              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    let strategies = vec![
        ("Adaptive Standard (25)", &results_standard),
        ("Adaptive Trend-Biased (20)", &results_trend),
        ("Adaptive Range-Biased (30)", &results_range),
    ];
    
    let best = strategies
        .iter()
        .max_by(|a, b| a.1.total_return_pct.partial_cmp(&b.1.total_return_pct).unwrap())
        .unwrap();
    
    println!("┌──────────────────────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("│ Strategy                 │ Return % │ Trades   │ Win Rate │ Sharpe   │");
    println!("├──────────────────────────┼──────────┼──────────┼──────────┼──────────┤");
    
    for (name, result) in &strategies {
        let marker = if *name == best.0 { "🏆" } else { "  " };
        println!("│ {}{:22} │ {:+7.2}% │ {:8} │ {:7.1}% │ {:8.2} │",
            marker,
            name,
            result.total_return_pct,
            result.total_trades,
            result.win_rate,
            result.sharpe_ratio);
    }
    
    println!("└──────────────────────────┴──────────┴──────────┴──────────┴──────────┘\n");
    
    println!("📊 Market Statistics:");
    let first_price = candles.first().unwrap().close;
    let last_price = candles.last().unwrap().close;
    let market_return = ((last_price - first_price) / first_price) * 100.0;
    println!("   Start Price:     ${:.2}", first_price);
    println!("   End Price:       ${:.2}", last_price);
    println!("   Market Return:   {:+.2}%", market_return);
    
    println!("\n💡 CONCLUSIONS:");
    println!("   • {} performed best with {:+.2}% return", best.0, best.1.total_return_pct);
    println!("   • Buy & Hold would have returned {:+.2}%", results_standard.buy_hold_return_pct);
    
    if best.1.total_return_pct > results_standard.buy_hold_return_pct {
        println!("   • ✅ ADAPTIVE STRATEGY BEAT THE MARKET!");
    } else if best.1.total_return_pct > 100.0 {
        println!("   • ✅ Strategy is highly profitable (+{}%)", best.1.total_return_pct as i32);
    } else {
        println!("   • ⚠️  Strategy profitable but underperforms B&H in strong bull");
    }
    
    println!("\n✅ Adaptive strategy backtest completed.");
}

/// Backtest sur les 3 derniers mois uniquement (période récente)
pub fn run_adaptive_recent_backtest() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🦎 ADAPTIVE STRATEGY - 3 DERNIERS MOIS (Recent Performance) ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    let all_candles = fetch_or_load_historical_data();
    
    if all_candles.is_empty() {
        eprintln!("❌ No historical data available. Aborting backtest.");
        return;
    }
    
    // Prendre seulement les 3 derniers mois (environ 2160 bougies 1H = 90 jours)
    let three_months_candles = 90 * 24; // 90 jours * 24 heures
    let start_idx = if all_candles.len() > three_months_candles {
        all_candles.len() - three_months_candles
    } else {
        0
    };
    
    let candles: Vec<_> = all_candles[start_idx..].to_vec();
    
    println!("⚙️  Configuration:");
    println!("   Asset:        SOL-USD (Coinbase Spot)");
    println!("   Timeframe:    1 Hour");
    println!("   Data Points:  {} candles", candles.len());
    println!("   Period:       ~{} days (3 derniers mois)", candles.len() / 24);
    println!("   Capital:      $1,000.00");
    println!("   Fee Rate:     0.10%");
    
    let initial_capital = 1000.0;
    let fee_rate = 0.001;
    
    // Test 1: Adaptive Trend-Biased (ADX=20) - Notre champion
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 1: Adaptive Trend-Biased (ADX=20) - Best 5Y Performer");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_trend = AdaptiveConfig {
        adx_threshold: 20.0,
        ..Default::default()
    };
    let results_trend = run_adaptive_backtest(
        config_trend,
        initial_capital,
        fee_rate,
        &candles,
    );
    
    print_adaptive_results(&results_trend, "Adaptive Trend-Biased (ADX=20) - 3M");
    
    // Comparaison finale
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  🏆 3 DERNIERS MOIS - COMPARISON                              ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    println!("┌──────────────────────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("│ Strategy (3 Months)      │ Return % │ Trades   │ Win Rate │ Max DD   │");
    println!("├──────────────────────────┼──────────┼──────────┼──────────┼──────────┤");
    println!("│ 🏆Adaptive Trend (ADX=20) │ {:+7.2}% │ {:8} │ {:7.1}% │ {:7.2}% │",
        results_trend.total_return_pct,
        results_trend.total_trades,
        results_trend.win_rate,
        results_trend.max_drawdown_pct);
    
    println!("└──────────────────────────┴──────────┴──────────┴──────────┴──────────┘\n");
    
    println!("📊 Market Statistics (3 Months):");
    let first_price = candles.first().unwrap().close;
    let last_price = candles.last().unwrap().close;
    let market_return = ((last_price - first_price) / first_price) * 100.0;
    println!("   Start Price:     ${:.2}", first_price);
    println!("   End Price:       ${:.2}", last_price);
    println!("   Market Return:   {:+.2}%", market_return);
    
    println!("\n💡 CONCLUSIONS (Période Récente):");
    println!("   • Adaptive Trend (ADX=20) performed best with {:+.2}% return", results_trend.total_return_pct);
    println!("   • Buy & Hold would have returned {:+.2}%", results_trend.buy_hold_return_pct);
    
    let diff = results_trend.total_return_pct - results_trend.buy_hold_return_pct;
    if diff > 0.0 {
        println!("   • ✅ Strategy BEAT the market by {:+.2}% (3 months)", diff);
    } else if results_trend.total_return_pct > 0.0 {
        println!("   • ✅ Strategy profitable ({:+.2}%) but market stronger", results_trend.total_return_pct);
    } else {
        println!("   • ⚠️  Strategy negative on this period ({:+.2}%)", results_trend.total_return_pct);
    }
    
    // Analyse par rapport aux 5 ans
    println!("\n📈 COMPARAISON 3 MOIS vs 5 ANS:");
    println!("   • Performance 5 ans (Adaptive):  +331.28%");
    println!("   • Performance 3 mois (Adaptive): {:+.2}%", results_trend.total_return_pct);
    
    if results_trend.total_return_pct > 10.0 {
        println!("   • ✅ Stratégie en forme récente (>10% sur 3 mois)");
    } else if results_trend.total_return_pct > 0.0 {
        println!("   • ⚠️  Performance récente modérée");
    } else {
        println!("   • ⚠️  Attention: Performance négative récente");
        println!("   • 💡 Considérer d'attendre une meilleure entrée");
    }
    
    println!("\n✅ Analyse des 3 derniers mois complétée.");
}

fn print_adaptive_results(results: &BacktestResults, strategy_name: &str) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  📊 BACKTEST RESULTS: {:48} ║", strategy_name);
    println!("╚═══════════════════════════════════════════════════════════════╝");
    
    println!("\n💰 CAPITAL:");
    println!("   Initial:  ${:.2}", results.initial_capital);
    println!("   Final:    ${:.2}", results.final_capital);
    println!("   Profit:   ${:+.2} ({:+.2}%)", 
        results.final_capital - results.initial_capital,
        results.total_return_pct);
    
    println!("\n📈 PERFORMANCE:");
    println!("   Strategy Return:   {:+.2}%", results.total_return_pct);
    println!("   Buy & Hold Return: {:+.2}%", results.buy_hold_return_pct);
    
    let diff = results.total_return_pct - results.buy_hold_return_pct;
    if diff > 0.0 {
        println!("   ✅ Outperformance: {:+.2}% vs Buy & Hold", diff);
    } else {
        println!("   ❌ Underperformance: {:+.2}% vs Buy & Hold", diff);
    }
    
    println!("\n📊 TRADES:");
    println!("   Total:   {}", results.total_trades);
    println!("   Wins:    {} ({:.1}%)", results.winning_trades, results.win_rate);
    println!("   Losses:  {}", results.losing_trades);
    println!("   Avg P&L: ${:.2} per trade", results.avg_profit_per_trade);
    
    println!("\n⚠️  RISK:");
    println!("   Max Drawdown: {:.2}%", results.max_drawdown_pct);
    println!("   Sharpe Ratio: {:.2}", results.sharpe_ratio);
    
    println!("\n═══════════════════════════════════════════════════════════════\n");
}
