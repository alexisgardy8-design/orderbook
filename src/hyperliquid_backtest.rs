// 📊 Backtest pour la stratégie Adaptive sur données Hyperliquid
// Compare performance sur SOL-PERP avec données réelles du DEX

use crate::adaptive_strategy::{AdaptiveStrategy, AdaptiveConfig, Signal};
use crate::hyperliquid_historical::{HyperliquidHistoricalData, HyperCandle};

/// Résultats d'un backtest
#[derive(Debug, Clone)]
pub struct HyperliquidBacktestResults {
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
    pub data_points: usize,
}

/// Structure pour stocker les résultats d'un trade
#[derive(Debug, Clone)]
struct Trade {
    entry_price: f64,
    exit_price: f64,
    profit_pct: f64,
    profit_usd: f64,
    trade_type: String, // "Range", "Trend", "Short"
}

/// Exécute le backtest de la stratégie adaptative sur données Hyperliquid
pub fn run_hyperliquid_adaptive_backtest(
    config: AdaptiveConfig,
    initial_capital: f64,
    fee_rate: f64,
    slippage_rate: f64,
    hourly_funding_rate: f64,
    candles: &[HyperCandle],
    stop_loss_pct: f64,
) -> HyperliquidBacktestResults {
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
    let mut sl_hits = 0;

    for candle in candles {
        // Convertir les prix strings en f64
        let high = match candle.h.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let low = match candle.l.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let close = match candle.c.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let open = match candle.o.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        // � FUNDING FEES (Deducted every hour if position is open)
        if position_size > 0.0 {
            let position_value = position_size * close;
            let funding_cost = position_value * hourly_funding_rate;
            capital -= funding_cost;
        }

        let is_bullish = close >= open;
        let mut exited_this_candle = false;

        // 🛡️ LOGIQUE D'EXÉCUTION INTRA-BOUGIE (High/Low Ordering)
        if is_bullish {
            // 1. Check LOW events first (SL for Long)
            if position_size > 0.0 && !is_short {
                let sl_price = entry_price * (1.0 - stop_loss_pct);
                if low <= sl_price {
                    let exit_price = sl_price * (1.0 - slippage_rate);
                    let revenue = position_size * exit_price;
                    let fee = revenue * fee_rate;
                    let net_revenue = revenue - fee;
                    capital += net_revenue;
                    let profit_usd = net_revenue - (position_size * entry_price);
                    let profit_pct = (profit_usd / (position_size * entry_price)) * 100.0;
                    trades.push(Trade {
                        entry_price,
                        exit_price,
                        profit_pct,
                        profit_usd,
                        trade_type: format!("{} (SL)", entry_type),
                    });
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                    sl_hits += 1;
                    exited_this_candle = true;
                }
            }

            // 2. Check HIGH events second (SL for Short)
            if !exited_this_candle && position_size > 0.0 && is_short {
                let sl_price = entry_price * (1.0 + stop_loss_pct);
                if high >= sl_price {
                    let exit_price = sl_price * (1.0 + slippage_rate);
                    let cost_to_cover = position_size * exit_price;
                    let fee = cost_to_cover * fee_rate;
                    let total_cost = cost_to_cover + fee;
                    let revenue = position_size * entry_price;
                    let profit_usd = revenue - total_cost;
                    let profit_pct = (profit_usd / revenue) * 100.0;
                    capital += profit_usd;
                    trades.push(Trade {
                        entry_price,
                        exit_price,
                        profit_pct,
                        profit_usd,
                        trade_type: format!("{} (SL)", entry_type),
                    });
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                    sl_hits += 1;
                    exited_this_candle = true;
                }
            }

            // 3. Update Strategy (Checks High for TP/Signal)
            let signal = strategy.update(high, low, close);
            
            if !exited_this_candle {
                process_signal(signal, &mut position_size, &mut capital, &mut is_short, &mut entry_price, &mut entry_type, &mut trades, &mut range_trades, &mut trend_trades, &mut short_trades, &mut upgrades, fee_rate, slippage_rate, close);
            }

        } else {
            // BEARISH (Open -> High -> Low -> Close)
            
            // 1. Check HIGH events first (SL for Short)
            if position_size > 0.0 && is_short {
                let sl_price = entry_price * (1.0 + stop_loss_pct);
                if high >= sl_price {
                    let exit_price = sl_price * (1.0 + slippage_rate);
                    let cost_to_cover = position_size * exit_price;
                    let fee = cost_to_cover * fee_rate;
                    let total_cost = cost_to_cover + fee;
                    let revenue = position_size * entry_price;
                    let profit_usd = revenue - total_cost;
                    let profit_pct = (profit_usd / revenue) * 100.0;
                    capital += profit_usd;
                    trades.push(Trade {
                        entry_price,
                        exit_price,
                        profit_pct,
                        profit_usd,
                        trade_type: format!("{} (SL)", entry_type),
                    });
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                    sl_hits += 1;
                    exited_this_candle = true;
                }
            }

            // 2. Check Strategy Exit (High)
            let signal = strategy.update(high, low, close);
            
            if !exited_this_candle {
                let is_exit = match signal {
                    Signal::SellRange | Signal::SellTrend | Signal::CoverShort => true,
                    _ => false
                };

                if is_exit && position_size > 0.0 {
                    process_signal(signal, &mut position_size, &mut capital, &mut is_short, &mut entry_price, &mut entry_type, &mut trades, &mut range_trades, &mut trend_trades, &mut short_trades, &mut upgrades, fee_rate, slippage_rate, close);
                    exited_this_candle = true;
                }
            }

            // 3. Check LOW events second (SL for Long)
            if !exited_this_candle && position_size > 0.0 && !is_short {
                let sl_price = entry_price * (1.0 - stop_loss_pct);
                if low <= sl_price {
                    let exit_price = sl_price * (1.0 - slippage_rate);
                    let revenue = position_size * exit_price;
                    let fee = revenue * fee_rate;
                    let net_revenue = revenue - fee;
                    capital += net_revenue;
                    let profit_usd = net_revenue - (position_size * entry_price);
                    let profit_pct = (profit_usd / (position_size * entry_price)) * 100.0;
                    trades.push(Trade {
                        entry_price,
                        exit_price,
                        profit_pct,
                        profit_usd,
                        trade_type: format!("{} (SL)", entry_type),
                    });
                    position_size = 0.0;
                    entry_type = String::from("None");
                    is_short = false;
                    sl_hits += 1;
                    exited_this_candle = true;
                }
            }

            // 4. Process Entry Signals (if not exited)
            if !exited_this_candle {
                match signal {
                    Signal::BuyRange | Signal::BuyTrend | Signal::SellShort | Signal::UpgradeToTrend => {
                         process_signal(signal, &mut position_size, &mut capital, &mut is_short, &mut entry_price, &mut entry_type, &mut trades, &mut range_trades, &mut trend_trades, &mut short_trades, &mut upgrades, fee_rate, slippage_rate, close);
                    },
                    _ => {}
                }
            }
        }

        // Strategy updated in logic block above
        let price = close;

        // Calcul de la valeur du portfolio (BIDIRECTIONNEL)
        let portfolio_value = if is_short {
            // En short: capital + gain/perte latent(e)
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

    let final_value = capital + (position_size * candles.last().unwrap().c.parse::<f64>().unwrap_or(0.0));

    let first_price = candles.first().unwrap().c.parse::<f64>().unwrap_or(0.0);
    let last_price = candles.last().unwrap().c.parse::<f64>().unwrap_or(0.0);
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
    println!("   Stop Loss Hits: {} trades", sl_hits);

    HyperliquidBacktestResults {
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
        data_points: candles.len(),
    }
}

/// Affiche les résultats du backtest
fn print_hyperliquid_results(results: &HyperliquidBacktestResults, strategy_name: &str) {
    println!("\n📈 {} Results:", strategy_name);
    println!("   Initial Capital:     ${:.2}", results.initial_capital);
    println!("   Final Capital:       ${:.2}", results.final_capital);
    println!("   Total Return:        {:+.2}%", results.total_return_pct);
    println!("   Total Trades:        {}", results.total_trades);
    println!("   Winning Trades:      {} ({:.1}%)", results.winning_trades, results.win_rate);
    println!("   Losing Trades:       {}", results.losing_trades);
    println!("   Avg Profit/Trade:    ${:.2}", results.avg_profit_per_trade);
    println!("   Max Drawdown:        {:.2}%", results.max_drawdown_pct);
    println!("   Sharpe Ratio:        {:.2}", results.sharpe_ratio);
    println!("   Buy & Hold Return:   {:+.2}%", results.buy_hold_return_pct);
}

/// Backtest complet avec données historiques Hyperliquid (SOL-PERP)
pub fn run_hyperliquid_backtest() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🚀 ADAPTIVE STRATEGY BACKTEST (Hyperliquid SOL-PERP)        ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    
    // Récupérer les données historiques depuis Hyperliquid
    println!("🌐 Fetching extended data from Hyperliquid API...");
    let client = HyperliquidHistoricalData::new("SOL".to_string(), "1h".to_string());
    
    // Force 1 year of data as requested
    let candles = match client.fetch_one_year() {
        Ok(data) if !data.is_empty() => {
            println!("✅ Successfully fetched {} candles (1 year)", data.len());
            data
        }
        _ => {
            println!("⏮️  Falling back to recent data...");
            match client.fetch_recent_candles() {
                Ok(data) => {
                    println!("✅ Successfully fetched {} candles (recent)", data.len());
                    data
                }
                Err(e) => {
                    eprintln!("❌ Failed to fetch any data: {}", e);
                    return;
                }
            }
        }
    };
    
    if candles.is_empty() {
        eprintln!("❌ No candles received from Hyperliquid. Aborting backtest.");
        return;
    }
    
    println!("⚙️  Configuration:");
    println!("   Asset:        SOL-PERP (Hyperliquid DEX)");
    println!("   Timeframe:    1 Hour");
    println!("   Data Points:  {} candles", candles.len());
    println!("   Period:       ~{} days (~{} years)", candles.len() / 24, (candles.len() / 24) / 365);
    println!("   Capital:      $1,000.00");
    println!("   Fee Rate:     0.05% (Hyperliquid maker fee)");
    println!("   Stop Loss:    1.0%");
    
    // Note: Hyperliquid maker fees are lower than Coinbase (0.02-0.05% vs 0.10%)
    let initial_capital = 1000.0;
    let fee_rate = 0.0005; // 0.05% - maker fee on Hyperliquid
    let slippage_rate = 0.001; // 0.1% - estimated slippage
    let hourly_funding_rate = 0.0000125; // ~10% APR / 365 / 24
    let stop_loss_pct = 0.01; // 1% Stop Loss (Optimized)
    
    // Test 1: Stratégie Adaptive Standard
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 1: Adaptive Strategy (ADX Threshold = 20)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_standard = AdaptiveConfig::default();
    let results_standard = run_hyperliquid_adaptive_backtest(
        config_standard,
        initial_capital,
        fee_rate,
        slippage_rate,
        hourly_funding_rate,
        &candles,
        stop_loss_pct,
    );
    
    print_hyperliquid_results(&results_standard, "Adaptive Standard (ADX=20)");
    
    // Test 2: Adaptive avec seuil ADX plus strict (favorise Trend)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 2: Adaptive Strategy - Trend Biased (ADX Threshold = 15)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_trend_biased = AdaptiveConfig {
        adx_threshold: 15.0, // Plus facile de passer en mode Trend
        ..Default::default()
    };
    let results_trend = run_hyperliquid_adaptive_backtest(
        config_trend_biased.clone(),
        initial_capital,
        fee_rate,
        slippage_rate,
        hourly_funding_rate,
        &candles,
        stop_loss_pct,
    );
    
    print_hyperliquid_results(&results_trend, "Adaptive Trend-Biased (ADX=15)");
    
    // Test 3: Adaptive avec seuil ADX plus lâche (favorise Range)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("TEST 3: Adaptive Strategy - Range Biased (ADX Threshold = 25)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let config_range_biased = AdaptiveConfig {
        adx_threshold: 25.0, // Plus strict, reste en mode Range plus souvent
        ..Default::default()
    };
    let results_range = run_hyperliquid_adaptive_backtest(
        config_range_biased,
        initial_capital,
        fee_rate,
        slippage_rate,
        hourly_funding_rate,
        &candles,
        stop_loss_pct,
    );
    
    print_hyperliquid_results(&results_range, "Adaptive Range-Biased (ADX=25)");

    // 🔍 OPTIMIZATION: Find Best Combination (ADX Threshold + Stop Loss)
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 OPTIMIZATION: Finding Best Combination (ADX + SL)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut best_combo_score = -999.0;
    let mut best_combo_desc = String::new();
    let mut best_combo_results = results_standard.clone();

    // Test ADX Thresholds: 10, 15, 20, 25, 30
    let adx_thresholds = vec![10.0, 15.0, 20.0, 25.0, 30.0];
    
    // Test SL from 1% to 10%
    let sl_values: Vec<f64> = (1..=10).map(|x| x as f64 / 100.0).collect();

    println!("Testing {} combinations...", adx_thresholds.len() * sl_values.len());
    println!("Score Formula: Return * 0.6 + Sharpe * 20.0 - MaxDrawdown * 1.5");

    for &adx in &adx_thresholds {
        for &sl in &sl_values {
            let config = AdaptiveConfig {
                adx_threshold: adx,
                ..Default::default()
            };
            
            let res = run_hyperliquid_adaptive_backtest(
                config,
                initial_capital,
                fee_rate,
                slippage_rate,
                hourly_funding_rate,
                &candles,
                sl,
            );
            
            // Custom Score: Balance Return, Risk (Sharpe), and Safety (Drawdown)
            // Return is %, e.g., 100.0
            // Sharpe is ratio, e.g., 0.1
            // Drawdown is %, e.g., 20.0
            let score = (res.total_return_pct * 0.6) + (res.sharpe_ratio * 20.0) - (res.max_drawdown_pct * 1.5);
            
            if score > best_combo_score {
                best_combo_score = score;
                best_combo_desc = format!("ADX={:.0} + SL={:.0}%", adx, sl * 100.0);
                best_combo_results = res.clone();
                println!("✨ New Best: {} -> Return: {:+.2}% | Sharpe: {:.2} | DD: {:.2}% | Score: {:.2}", 
                    best_combo_desc, res.total_return_pct, res.sharpe_ratio, res.max_drawdown_pct, score);
            }
        }
    }

    println!("\n🏆 ULTIMATE WINNER: {}", best_combo_desc);
    print_hyperliquid_results(&best_combo_results, &format!("Ultimate Optimized ({})", best_combo_desc));
    
    // Comparaison finale
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  🏆 HYPERLIQUID ADAPTIVE STRATEGY COMPARISON                  ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    let strategies = vec![
        ("Adaptive Standard (20)", &results_standard),
        ("Adaptive Trend-Biased (15)", &results_trend),
        ("Adaptive Range-Biased (25)", &results_range),
        ("🏆 Ultimate Optimized", &best_combo_results),
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
    let first_price = candles.first().unwrap().c.parse::<f64>().unwrap_or(0.0);
    let last_price = candles.last().unwrap().c.parse::<f64>().unwrap_or(0.0);
    let market_return = ((last_price - first_price) / first_price) * 100.0;
    println!("   Start Price:     ${:.2}", first_price);
    println!("   End Price:       ${:.2}", last_price);
    println!("   Market Return:   {:+.2}%", market_return);
    println!("   Period Duration: {} days", candles.len() / 24);
    
    println!("\n💡 CONCLUSIONS:");
    println!("   • {} performed best with {:+.2}% return", best.0, best.1.total_return_pct);
    println!("   • Buy & Hold would have returned {:+.2}%", results_standard.buy_hold_return_pct);
    
    if best.1.total_return_pct > results_standard.buy_hold_return_pct {
        println!("   • ✅ ADAPTIVE STRATEGY BEAT THE MARKET!");
        let outperformance = best.1.total_return_pct - results_standard.buy_hold_return_pct;
        println!("   • Outperformance: +{:.2}%", outperformance);
    } else if best.1.total_return_pct > 0.0 {
        println!("   • ✅ Strategy is profitable but underperforms B&H");
        let underperformance = results_standard.buy_hold_return_pct - best.1.total_return_pct;
        println!("   • Underperformance: -{:.2}%", underperformance);
    } else {
        println!("   • ⚠️  Strategy is unprofitable in this period");
    }
    
    // Afficher les détails du meilleur backtest
    println!("\n🔍 BEST CONFIGURATION DETAILS:");
    println!("   Maximum Drawdown: {:.2}%", best.1.max_drawdown_pct);
    println!("   Sharpe Ratio:     {:.2}", best.1.sharpe_ratio);
    println!("   Win Rate:         {:.1}%", best.1.win_rate);
    
    if best.1.total_trades > 0 {
        println!("   Avg Trade Profit: ${:.2}", best.1.avg_profit_per_trade);
        println!("   Profit Factor:    {:.2}x", best.1.final_capital / results_standard.initial_capital);
    }
    
    println!("\n✅ Hyperliquid backtest completed.");
    println!("📌 Ready for live trading deployment!\n");
}

fn process_signal(
    signal: Signal,
    position_size: &mut f64,
    capital: &mut f64,
    is_short: &mut bool,
    entry_price: &mut f64,
    entry_type: &mut String,
    trades: &mut Vec<Trade>,
    range_trades: &mut i32,
    trend_trades: &mut i32,
    short_trades: &mut i32,
    upgrades: &mut i32,
    fee_rate: f64,
    slippage_rate: f64,
    price: f64,
) {
    match signal {
        Signal::BuyRange => {
            if *position_size == 0.0 && !*is_short {
                let amount_to_invest = *capital * 0.95;
                let fee = amount_to_invest * fee_rate;
                let cost = amount_to_invest + fee;
                
                if cost <= *capital {
                    *position_size = amount_to_invest / price;
                    *capital -= cost;
                    *entry_price = price;
                    *entry_type = String::from("Range");
                    *range_trades += 1;
                    *is_short = false;
                }
            }
        }
        Signal::BuyTrend => {
            if *position_size == 0.0 && !*is_short {
                let amount_to_invest = *capital * 0.95;
                let fee = amount_to_invest * fee_rate;
                let cost = amount_to_invest + fee;
                
                if cost <= *capital {
                    *position_size = amount_to_invest / price;
                    *capital -= cost;
                    *entry_price = price;
                    *entry_type = String::from("Trend");
                    *trend_trades += 1;
                    *is_short = false;
                }
            }
        }
        Signal::SellShort => {
            if *position_size == 0.0 && !*is_short {
                let amount_to_invest = *capital * 0.95;
                let fee = amount_to_invest * fee_rate;
                let cost = amount_to_invest + fee;
                
                if cost <= *capital {
                    *position_size = amount_to_invest / price;
                    *capital -= fee;
                    *entry_price = price;
                    *entry_type = String::from("Short");
                    *short_trades += 1;
                    *is_short = true;
                }
            }
        }
        Signal::SellRange | Signal::SellTrend => {
            if *position_size > 0.0 && !*is_short {
                let exit_price = price * (1.0 - slippage_rate);
                let revenue = *position_size * exit_price;
                let fee = revenue * fee_rate;
                let net_revenue = revenue - fee;
                
                *capital += net_revenue;
                
                let profit_usd = net_revenue - (*position_size * *entry_price);
                let profit_pct = (profit_usd / (*position_size * *entry_price)) * 100.0;
                
                trades.push(Trade {
                    entry_price: *entry_price,
                    exit_price,
                    profit_pct,
                    profit_usd,
                    trade_type: entry_type.clone(),
                });
                
                *position_size = 0.0;
                *entry_type = String::from("None");
                *is_short = false;
            }
        }
        Signal::CoverShort => {
            if *position_size > 0.0 && *is_short {
                let exit_price = price * (1.0 + slippage_rate);
                let cost_to_cover = *position_size * exit_price;
                let fee = cost_to_cover * fee_rate;
                let total_cost = cost_to_cover + fee;
                
                let revenue = *position_size * *entry_price;
                let profit_usd = revenue - total_cost;
                let profit_pct = (profit_usd / revenue) * 100.0;
                
                *capital += profit_usd;
                
                trades.push(Trade {
                    entry_price: *entry_price,
                    exit_price,
                    profit_pct,
                    profit_usd,
                    trade_type: entry_type.clone(),
                });
                
                *position_size = 0.0;
                *entry_type = String::from("None");
                *is_short = false;
            }
        }
        Signal::UpgradeToTrend => {
            *entry_type = String::from("Trend");
            *upgrades += 1;
        }
        Signal::Hold => {}
    }
}
