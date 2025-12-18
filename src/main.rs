use crate::{benchmarks::OrderBookBenchmark, orderbook::OrderBookImpl};

mod benchmarks;
mod interfaces;
mod orderbook;
mod data_loader;
mod triangular_arbitrage;
mod backtest;
mod reporting;
mod arbitrage_benchmark;
mod adaptive_strategy;
mod hyperliquid_historical;
mod hyperliquid_backtest;
mod position_manager;
mod order_executor;

#[cfg(feature = "websocket")]
mod supabase;

#[cfg(feature = "websocket")]
mod telegram;

#[cfg(feature = "websocket")]
mod hyperliquid_feed;

#[cfg(feature = "websocket")]
mod test_live_order;

#[cfg(feature = "websocket")]
mod test_sl_order;

#[cfg(feature = "websocket")]
mod test_supabase_log;

#[cfg(feature = "websocket")]
mod test_market_cycle;

#[cfg(feature = "websocket")]
mod test_real_pnl;

#[cfg(feature = "websocket")]
mod hyperliquid_trade;

// Legacy modules (kept for reference but not used)
// mod coinbase_feed;
// mod coinbase_historical;
// mod adaptive_backtest;

use std::env;

fn main() {
    // Load .env file
    dotenv::dotenv().ok();

    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            #[cfg(feature = "websocket")]
            "test-order" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = test_live_order::run_test_order_execution().await {
                        eprintln!("❌ Test order error: {}", e);
                    }
                });
            }
            #[cfg(feature = "websocket")]
            "test-sl" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = test_sl_order::run_test_sl_order().await {
                        eprintln!("❌ Test SL error: {}", e);
                    }
                });
            }
            #[cfg(feature = "websocket")]
            "test-cycle" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = test_market_cycle::run_test_market_cycle().await {
                        eprintln!("❌ Test Cycle error: {}", e);
                    }
                });
            }
            #[cfg(feature = "websocket")]
            "test-pnl" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    if let Err(e) = test_real_pnl::run_test_pnl().await {
                        eprintln!("❌ Test PnL error: {}", e);
                    }
                });
            }
            #[cfg(feature = "websocket")]
            "test-supabase" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    println!("🗄️ Testing Supabase Connection...");
                    if let Some(client) = supabase::SupabaseClient::new() {
                        println!("✅ Supabase Client initialized.");
                        
                        // Test Log
                        match client.log("INFO", "Test connection from CLI", Some("test-supabase")).await {
                            Ok(_) => println!("✅ Log entry created successfully."),
                            Err(e) => eprintln!("❌ Failed to create log: {}", e),
                        }

                        // Test Fetch Positions
                        match client.fetch_open_positions().await {
                            Ok(positions) => println!("✅ Fetched {} open positions.", positions.len()),
                            Err(e) => eprintln!("❌ Failed to fetch positions: {}", e),
                        }

                    } else {
                        eprintln!("❌ Supabase not configured. Check .env file (SUPABASE_URL, SUPABASE_KEY).");
                    }
                });
            }
            #[cfg(feature = "websocket")]
            "test-telegram" => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    println!("🤖 Testing Telegram Bot...");
                    if let Some(bot) = telegram::TelegramBot::new() {
                        println!("✅ Telegram Bot configured.");
                        
                        // 1. Send simple message
                        match bot.send_message("🔔 *Test Notification*\n\nCeci est un test du bot de trading Rust.").await {
                            Ok(_) => println!("✅ Message sent successfully! Check your Telegram."),
                            Err(e) => eprintln!("❌ Failed to send message: {}", e),
                        }

                        // 2. Send Control Keyboard
                        println!("⌨️  Sending Control Keyboard...");
                        match bot.send_control_keyboard(true).await {
                            Ok(_) => println!("✅ Control Keyboard sent! Check your Telegram."),
                            Err(e) => eprintln!("❌ Failed to send keyboard: {}", e),
                        }

                        // 3. Start Listener for interaction
                        println!("👂 Starting Listener for button clicks (Press Ctrl+C to stop)...");
                        let is_running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                        let position_manager = std::sync::Arc::new(tokio::sync::Mutex::new(position_manager::PositionManager::new(1000.0, None)));
                        
                        // Create dummy channel for testing
                        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(100);
                        
                        // Spawn a task to print received commands
                        tokio::spawn(async move {
                            while let Some(cmd) = cmd_rx.recv().await {
                                println!("🧪 Test received command: {:?}", cmd);
                            }
                        });

                        bot.run_listener(is_running, position_manager, cmd_tx).await;

                    } else {
                        eprintln!("❌ Telegram Bot not configured. Check .env file.");
                    }
                });
            }
            "test" => test_hyperliquid(),
            "trade" => {
                #[cfg(feature = "websocket")]
                {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        if let Err(e) = hyperliquid_feed::run_live_trading().await {
                            eprintln!("❌ Live trading error: {}", e);
                        }
                    });
                }
                
                #[cfg(not(feature = "websocket"))]
                {
                    eprintln!("❌ WebSocket feature not enabled");
                    eprintln!("💡 Compile with: cargo build --release --features websocket");
                }
            }
            "hl-backtest" => run_hyperliquid_backtest(),
            "backtest" => run_backtest(),
            "perf" => run_arbitrage_performance(),
            _ => run_benchmark(),
        }
    } else {
        run_benchmark();
    }
}

fn run_arbitrage_performance() {
    arbitrage_benchmark::ArbitrageBenchmark::run_detection_benchmark();
}

fn run_hyperliquid_backtest() {
    hyperliquid_backtest::run_hyperliquid_backtest();
}

fn test_hyperliquid() {
    hyperliquid_historical::test_hyperliquid_connection();
}

fn run_benchmark() {
    println!("Running Naive OrderBook Benchmark...\n");

    let result = OrderBookBenchmark::run::<OrderBookImpl>("OrderBook", 100_000);
    OrderBookBenchmark::print_results(&result);

    println!("\n Competition Goal: Achieve sub-nanosecond operations!");
    println!(" Tips:");
    println!("   - Use cache-friendly data structures");
    println!("   - Consider BTreeMap for sorted access");
    println!("   - Pre-allocate where possible");
    println!("   - Profile with 'cargo flamegraph'");
    println!("   - Use 'cargo bench' for micro-benchmarks");
}

fn run_backtest() {
    println!("🚀 Starting Triangular Arbitrage Backtest\n");
    
    println!("═══════════════════════════════════════════════════════════");
    println!("  CONFIGURATION");
    println!("═══════════════════════════════════════════════════════════");
    println!("Triangle: ETH-BTC-USDC (Highest liquidity on Coinbase)");
    println!("  • pair1: ETH-USDC  (precision: 4 decimals, factor 10,000)");
    println!("  • pair2: BTC-USDC  (precision: 4 decimals, factor 10,000)");
    println!("  • pair3: ETH-BTC   (precision: 8 decimals, factor 100,000,000)");
    println!();
    println!("Paths:");
    println!("  • Forward: USDC → ETH → BTC → USDC");
    println!("  • Reverse: USDC → BTC → ETH → USDC");
    println!();
    println!("Parameters:");
    println!("  • Minimum profit threshold: 2.0 bps (0.02%)");
    println!("  • Starting capital: $1,000.00");
    println!("  • Trading fee: 0.1% per transaction");
    println!("═══════════════════════════════════════════════════════════\n");
    
    println!("📥 Generating realistic market data...");
    let pair1_data = data_loader::DataLoader::generate_realistic_arbitrage_data(
        "ETH-USDC", 3000, 3146.0, 0.015
    );
    let pair2_data = data_loader::DataLoader::generate_realistic_arbitrage_data(
        "BTC-USDC", 3000, 89903.62, 0.01
    );
    let pair3_data = data_loader::DataLoader::generate_realistic_arbitrage_data(
        "ETH-BTC", 3000, 0.03499, 0.02
    );
    
    println!("  ✅ Generated {} updates for ETH-USDC", pair1_data.len());
    println!("  ✅ Generated {} updates for BTC-USDC", pair2_data.len());
    println!("  ✅ Generated {} updates for ETH-BTC", pair3_data.len());
    println!("  ✅ Total: {} market updates", pair1_data.len() + pair2_data.len() + pair3_data.len());
    
    println!("\n🔍 Running ultra-fast backtest simulation...");
    
    let mut engine = backtest::BacktestEngine::new(2.0, 1000.0);
    let result = engine.run(pair1_data, pair2_data, pair3_data);
    
    reporting::ReportGenerator::print_backtest_report(&result);
    
    let ns_per_update = (result.execution_time_ms as f64 * 1_000_000.0) / result.total_updates_processed as f64;
    println!("\n⚡ Performance Analysis:");
    println!("   Nanoseconds per update:     {:.3} ns", ns_per_update);
    if ns_per_update < 1.0 {
        println!("   ✅ TARGET ACHIEVED: Sub-nanosecond operation!");
    } else {
        println!("   ⚠️  Target: <1ns (current: {:.3}ns)", ns_per_update);
    }
    
    println!("\n💡 Note on Results:");
    if result.total_opportunities == 0 {
        println!("   No arbitrage opportunities found - This is expected!");
        println!("   Real market prices are well-aligned on liquid pairs.");
        println!("   Opportunities occur during:");
        println!("     • High volatility periods");
        println!("     • Major news announcements");
        println!("     • Large liquidation cascades");
        println!("     • Flash crashes");
    }
    
    println!("\n💾 Saving report to file...");
    if let Err(e) = reporting::ReportGenerator::generate_csv_report(&result, "backtest_report.csv") {
        eprintln!("Failed to save report: {}", e);
    } else {
        println!("  ✅ Report saved to backtest_report.csv");
    }
}

// DEPRECATED: Coinbase live mode - Use 'trade' command for Hyperliquid instead
// fn run_live_mode() {
//     println!("❌ Legacy Coinbase mode removed. Use: cargo run --release --features websocket trade");
// }

// ============================================================================
// CORRECTNESS TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use crate::{
        interfaces::{OrderBook, Side, Update},
        orderbook::OrderBookImpl,
    };

    fn test_basic_operations<T: OrderBook>() {
        let mut ob = T::new();

        // Add bids
        ob.apply_update(Update::Set {
            price: 10000,
            quantity: 100,
            side: Side::Bid,
        });
        ob.apply_update(Update::Set {
            price: 9950,
            quantity: 150,
            side: Side::Bid,
        });

        // Add asks
        ob.apply_update(Update::Set {
            price: 10050,
            quantity: 80,
            side: Side::Ask,
        });
        ob.apply_update(Update::Set {
            price: 10100,
            quantity: 120,
            side: Side::Ask,
        });

        assert_eq!(ob.get_best_bid(), Some(10000));
        assert_eq!(ob.get_best_ask(), Some(10050));
        assert_eq!(ob.get_spread(), Some(50));
        assert_eq!(ob.get_quantity_at(10000, Side::Bid), Some(100));
    }

    fn test_updates_and_removes<T: OrderBook>() {
        let mut ob = T::new();

        ob.apply_update(Update::Set {
            price: 10000,
            quantity: 100,
            side: Side::Bid,
        });
        assert_eq!(ob.get_quantity_at(10000, Side::Bid), Some(100));

        // Update quantity
        ob.apply_update(Update::Set {
            price: 10000,
            quantity: 200,
            side: Side::Bid,
        });
        assert_eq!(ob.get_quantity_at(10000, Side::Bid), Some(200));

        // Remove via zero quantity
        ob.apply_update(Update::Set {
            price: 10000,
            quantity: 0,
            side: Side::Bid,
        });
        assert_eq!(ob.get_quantity_at(10000, Side::Bid), None);

        // Remove via Remove update
        ob.apply_update(Update::Set {
            price: 10000,
            quantity: 100,
            side: Side::Bid,
        });
        ob.apply_update(Update::Remove {
            price: 10000,
            side: Side::Bid,
        });
        assert_eq!(ob.get_quantity_at(10000, Side::Bid), None);
    }

    #[test]
    fn test_naive_implementation() {
        test_basic_operations::<OrderBookImpl>();
        test_updates_and_removes::<OrderBookImpl>();
    }
}
