mod benchmarks;
mod interfaces;
mod orderbook;

use crate::benchmarks::OrderBookBenchmark;
use crate::orderbook::OrderBookImpl;

fn main() {
    println!("🚀 Starting OrderBook Benchmark");
    
    // Run benchmark with 100,000 iterations
    let result = OrderBookBenchmark::run::<OrderBookImpl>("OrderBookImpl", 100_000);
    
    println!("============================================================");
    println!("  BENCHMARK RESULTS: {}", result.name);
    println!("============================================================");
    println!("  Total Operations: {}", result.total_operations);
    println!("  ---");
    println!("  Update Operations:");
    println!("    Average: {:.2} ns", result.avg_update_ns);
    println!("    P50:     {} ns", result.p50_update_ns);
    println!("    P95:     {} ns", result.p95_update_ns);
    println!("    P99:     {} ns", result.p99_update_ns);
    println!("  ---");
    println!("  Get Best Bid:");
    println!("    Average: {:.2} ns", result.avg_best_bid_ns);
    println!("  Get Best Ask:");
    println!("    Average: {:.2} ns", result.avg_best_ask_ns);
    println!("  Get Spread:");
    println!("    Average: {:.2} ns", result.avg_spread_ns);
    println!("  Random Reads:");
    println!("    Average: {:.2} ns", result.avg_random_read_ns);
    println!("============================================================");
}
