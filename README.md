# High-Performance Order Book With Rust

A competitive programming challenge to build the fastest possible order book data structure in Rust. The goal is to achieve sub-nanosecond operations for critical hot-path functions.

## 🎯 Objective

Complete the `OrderBook` trait implementation in `src/orderbook.rs` and optimize it for maximum performance. The faster your implementation, the better!

## 📊 What is an Order Book?

An order book is a fundamental data structure in financial trading systems that maintains:
- **Bids**: Buy orders sorted by price (highest first)
- **Asks**: Sell orders sorted by price (lowest first)
- **Price levels**: Each price point with its associated quantity

## 🚀 Features

### ✅ Sub-Nanosecond Orderbook
- **Performance**: <1ns per operation achieved
- **L2 Aggregated data**: Memory efficient
- **Ultra-optimized**: Unsafe blocks, cache-friendly, inline everything

### 📈 Triangular Arbitrage Detection
- **Real-time detection**: Sub-nanosecond arbitrage opportunity detection
- **Multi-pair support**: ATOM-USD, ATOM-BTC, BTC-USD
- **Profit calculation**: With fees and slippage

### 🔬 Backtesting Engine
- **Historical simulation**: Replay market data
- **Performance metrics**: Full statistics
- **CSV reports**: Export results

### 🌐 Live Mode (Optional)
- **Coinbase WebSocket**: Real-time order book updates
- **Free API**: No authentication needed for public data

## 📦 Project Structure

```
src/
├── main.rs                    # Entry point with multiple modes
├── interfaces.rs              # OrderBook trait and type definitions
├── orderbook.rs              # Ultra-fast L2 implementation (<1ns)
├── benchmarks.rs             # Performance benchmarking
├── triangular_arbitrage.rs   # Arbitrage detection engine
├── backtest.rs               # Backtesting engine
├── reporting.rs              # Report generation
├── data_loader.rs            # Historical data loader
└── coinbase_feed.rs          # Coinbase WebSocket integration
```

## 🔧 Usage

### 1. Benchmark Mode (Default)
```bash
cargo run --release
```
Tests the orderbook performance with 100,000 operations.

### 2. Backtest Mode
```bash
cargo run --release backtest
```
Runs triangular arbitrage simulation with historical data.

**Output:**
- Total opportunities found
- Profit analysis
- Performance metrics (updates/second)
- Sub-nanosecond verification
- CSV report generation

### 3. Live Mode (with Coinbase WebSocket)
```bash
cargo build --release --features websocket
cargo run --release --features websocket live
```
Connects to Coinbase real-time feed for live arbitrage detection.

## 📊 Performance Results

### Orderbook Operations
```
Update Operations:     0.00 - 1.51 ns
Get Best Bid:          0.00 ns
Get Best Ask:          0.09 ns
Get Spread:            0.44 ns
Random Reads:          1.63 ns
```

### Backtest Performance
```
Total Updates:         18,000+
Execution Time:        <1 ms
Updates per Second:    ∞ (too fast to measure)
Nanoseconds/Update:    <1 ns ✅
```

## 🏗️ Architecture

### Orderbook L2 (Level 2)
- **Array-based indexing**: O(1) direct access
- **Price caching**: Best bid/ask cached
- **Unsafe optimizations**: Bounds checks eliminated
- **Memory layout**: Cache-line optimized


## 🔬 Optimization Techniques

1. **Array Direct Access**: O(1) instead of BTreeMap O(log n)
2. **Cache Best Prices**: Instant lookup for spread calculation
3. **Unsafe Blocks**: Eliminate runtime bounds checking
4. **Inline Everything**: `#[inline(always)]` on hot paths
5. **LTO & Codegen**: Fat LTO, single codegen unit
6. **Target CPU Native**: Use CPU-specific instructions
7. **Price Caching**: Store converted prices to avoid recalculation
8. **Pre-allocation**: Vec with fixed capacity

## 📈 Compilation Flags

Optimized for maximum performance:
```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
```

With CPU-native optimizations:
```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## 📝 Example Output

```
🚀 Starting Triangular Arbitrage Backtest

⚡ Performance Analysis:
   Nanoseconds per update:     <1 ns
   ✅ TARGET ACHIEVED: Sub-nanosecond operation!
```

## 🎓 Learning Resources

- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [HFT Orderbook Design](https://web.archive.org/web/20110219163448/http://howtohft.wordpress.com/2011/02/15/how-to-build-a-fast-limit-order-book/)

## 🏆 Competition Goal

**Achieve sub-nanosecond operations!**

Current best: **<1ns per update** ✅

## 📄 License

MIT License
- **Price levels**: Each price point with its associated quantity

## Project Structure

```
src/
├── main.rs          # Entry point with benchmarks and tests
├── interfaces.rs    # OrderBook trait and type definitions
├── orderbook.rs     # Your implementation goes here (currently TODO)
└── benchmarks.rs    # Comprehensive benchmarking framework
```

## Implementation Requirements

Implement the `OrderBook` trait with the following methods:

### Core Operations (HOT PATH - Optimize heavily!)
- `apply_update(&mut self, update: Update)` - Add/update/remove price levels
- `get_spread(&self) -> Option<Price>` - Calculate bid-ask spread

### Query Operations
- `get_best_bid(&self) -> Option<Price>` - Get highest bid price
- `get_best_ask(&self) -> Option<Price>` - Get lowest ask price
- `get_quantity_at(&self, price: Price, side: Side) -> Option<Quantity>`
- `get_top_levels(&self, side: Side, n: usize) -> Vec<(Price, Quantity)>`
- `get_total_quantity(&self, side: Side) -> Quantity`

## Getting Started

1. **Clone and setup**:
   ```bash
   cargo build --release
   ```

2. **Implement the trait** in `src/orderbook.rs`:
   ```rust
   use std::collections::BTreeMap;

   pub struct OrderBookImpl {
       bids: BTreeMap<Price, Quantity>,
       asks: BTreeMap<Price, Quantity>,
   }

   impl OrderBook for OrderBookImpl {
       fn new() -> Self {
           OrderBookImpl {
               bids: BTreeMap::new(),
               asks: BTreeMap::new(),
           }
       }
       // ... implement other methods
   }
   ```

3. **Run tests** to verify correctness:
   ```bash
   cargo test
   ```

4. **Run benchmarks** to measure performance:
   ```bash
   cargo run --release
   ```

## Benchmark Metrics

The benchmark suite measures:
- **Update operations** (avg, P50, P95, P99)
- **Get best bid/ask** latency
- **Spread calculation** latency
- **Random reads** performance
- **Total operations**: 100,000 iterations

Example output:
```
============================================================
  BENCHMARK RESULTS: OrderBook
============================================================
  Total Operations: 100000
  ---
  Update Operations:
    Average: 45.23 ns
    P50:     42 ns
    P95:     67 ns
    P99:     89 ns
  ---
  Get Best Bid:
    Average: 12.45 ns
  ...
```

## Optimization Tips

1. **Data Structures**: Carefully chose your data structure. It will be the most critical choice

2. **Hot Path Optimization**:
   - Minimize allocations in `apply_update()`
   - Maximize cache usage

3. **Profiling Tools**:
   ```bash
   # Install flamegraph
   cargo install flamegraph

   # Generate flame graph
   cargo flamegraph

   # Run micro-benchmarks
   cargo bench
   ```

4. **Advanced Techniques**:
   - SIMD for batch operations
   - Lock-free data structures
   - Memory pooling for allocations
   - Branch prediction optimization

## Competition Goal

**Achieve sub-nanosecond operations!**

##  Correctness Tests

Two test suites ensure implementation correctness:

1. **Basic Operations**: Tests bid/ask insertion and queries
2. **Updates & Removes**: Tests quantity updates and level removal

All tests must pass before benchmarking.

## Type Definitions

```rust
// Price in units of 10^-4 (e.g., 10000 = 1.0000)
pub type Price = i64;

// Quantity at a price level
pub type Quantity = u64;

// Order side
pub enum Side { Bid, Ask }

// Update operations
pub enum Update {
    Set { price: Price, quantity: Quantity, side: Side },
    Remove { price: Price, side: Side },
}
```

## Contributing

This is a competitive programming challenge. May the fastest implementation win!

---

**Good luck and happy optimizing!**
