use crate::{benchmarks::OrderBookBenchmark, orderbook::OrderBookImpl};

mod benchmarks;
mod interfaces;
mod orderbook;
mod data_loader;
mod triangular_arbitrage;
mod backtest;
mod reporting;
mod coinbase_feed;
mod arbitrage_benchmark;
mod coinbase_historical;
mod adaptive_strategy;
mod adaptive_backtest;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        match args[1].as_str() {
            "adaptive" => run_adaptive_backtest(),
            "recent" => run_recent_backtest(),
            "backtest" => run_backtest(),
            "live" => run_live_mode(),
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

fn run_adaptive_backtest() {
    adaptive_backtest::run_adaptive_real_data_backtest();
}

fn run_recent_backtest() {
    adaptive_backtest::run_adaptive_recent_backtest();
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

fn run_live_mode() {
    #[cfg(feature = "websocket")]
    {
        println!("🌐 Starting Live Mode - Connecting to Coinbase...\n");
        
        println!("   Triangle: ETH-BTC-USDC");
        println!("   - Highest liquidity on Coinbase");
        println!("   - Institutional-grade pairs");
        println!("   - Microsecond-level opportunities\n");
        
        let products = vec![
            "ETH-USDC".to_string(),
            "BTC-USDC".to_string(),
            "ETH-BTC".to_string(),
        ];
        
        let feed = coinbase_feed::CoinbaseFeed::new(products);
        
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = feed.connect_with_arbitrage().await {
                eprintln!("❌ Connection error: {}", e);
            }
        });
    }
    
    #[cfg(not(feature = "websocket"))]
    {
        println!("❌ Live mode not available. Compile with --features websocket");
        println!("   cargo run --release --features websocket live");
    }
}

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
