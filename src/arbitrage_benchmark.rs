use crate::triangular_arbitrage::TriangularArbitrageDetector;
use crate::interfaces::{OrderBook, Side, Update};
use std::time::Instant;

pub struct ArbitrageBenchmark;

impl ArbitrageBenchmark {
    pub fn run_detection_benchmark() {
        println!("\n⚡ ARBITRAGE DETECTION PERFORMANCE BENCHMARK\n");
        
        let mut detector = TriangularArbitrageDetector::new(2.0);
        
        // Simuler un orderbook avec des prix réalistes
        Self::setup_realistic_orderbooks(&mut detector);
        
        // Mesurer l'overhead du timing
        let overhead = Self::measure_overhead();
        println!("🔬 Timing overhead: ~{} ns\n", overhead);
        
        // Benchmark 1: Détection simple (une seule opportunité)
        let single_detection_ns = Self::benchmark_single_detection(&mut detector, overhead);
        
        // Benchmark 2: Détection avec mise à jour du cache
        let with_cache_update_ns = Self::benchmark_with_cache_update(&mut detector, overhead);
        
        // Benchmark 3: Cycle complet (update orderbook + détection)
        let full_cycle_ns = Self::benchmark_full_cycle(&mut detector, overhead);
        
        // Afficher les résultats
        Self::print_results(single_detection_ns, with_cache_update_ns, full_cycle_ns);
    }
    
    fn measure_overhead() -> u64 {
        let iterations = 10000;
        let mut timings = Vec::with_capacity(iterations);
        
        for _ in 0..iterations {
            let start = Instant::now();
            std::hint::black_box(());
            let elapsed = start.elapsed().as_nanos() as u64;
            timings.push(elapsed);
        }
        
        timings.sort();
        timings[iterations / 2]
    }
    
    fn setup_realistic_orderbooks(detector: &mut TriangularArbitrageDetector) {
        // ETH-USDC: ~3146.0
        detector.pair1.apply_update(Update::Set {
            price: 31460000, // 3146.0 * 10000
            quantity: 10000,
            side: Side::Ask,
        });
        detector.pair1.apply_update(Update::Set {
            price: 31450000, // 3145.0 * 10000
            quantity: 10000,
            side: Side::Bid,
        });
        
        // BTC-USDC: ~89903.62
        detector.pair2.apply_update(Update::Set {
            price: 899036200, // 89903.62 * 10000
            quantity: 1000,
            side: Side::Ask,
        });
        detector.pair2.apply_update(Update::Set {
            price: 899000000, // 89900.0 * 10000
            quantity: 1000,
            side: Side::Bid,
        });
        
        // ETH-BTC: ~0.03499 (utilise 100000000 pour 8 décimales)
        detector.pair3.apply_update(Update::Set {
            price: 3500000, // 0.03500000 * 100000000
            quantity: 10000,
            side: Side::Ask,
        });
        detector.pair3.apply_update(Update::Set {
            price: 3498000, // 0.03498000 * 100000000
            quantity: 10000,
            side: Side::Bid,
        });
    }
    
    fn benchmark_single_detection(detector: &mut TriangularArbitrageDetector, overhead: u64) -> (f64, u64, u64, u64) {
        let iterations = 100_000;
        let mut timings = Vec::with_capacity(iterations);
        
        // Préchauffer le cache
        detector.update_price_cache();
        
        println!("📊 Benchmark 1: Détection simple (cache déjà à jour)");
        println!("   Iterations: {}", iterations);
        
        for _ in 0..iterations {
            let start = Instant::now();
            let _opportunities = detector.detect_opportunities(12345, 1000.0);
            let elapsed = start.elapsed().as_nanos() as u64;
            timings.push(elapsed.saturating_sub(overhead));
        }
        
        timings.sort();
        let avg = timings.iter().sum::<u64>() as f64 / timings.len() as f64;
        let p50 = timings[timings.len() / 2];
        let p95 = timings[timings.len() * 95 / 100];
        let p99 = timings[timings.len() * 99 / 100];
        
        println!("   ✅ Complete\n");
        (avg, p50, p95, p99)
    }
    
    fn benchmark_with_cache_update(detector: &mut TriangularArbitrageDetector, overhead: u64) -> (f64, u64, u64, u64) {
        let iterations = 100_000;
        let mut timings = Vec::with_capacity(iterations);
        
        println!("📊 Benchmark 2: Détection avec mise à jour du cache");
        println!("   Iterations: {}", iterations);
        
        for _ in 0..iterations {
            let start = Instant::now();
            detector.update_price_cache();
            let _opportunities = detector.detect_opportunities(12345, 1000.0);
            let elapsed = start.elapsed().as_nanos() as u64;
            timings.push(elapsed.saturating_sub(overhead));
        }
        
        timings.sort();
        let avg = timings.iter().sum::<u64>() as f64 / timings.len() as f64;
        let p50 = timings[timings.len() / 2];
        let p95 = timings[timings.len() * 95 / 100];
        let p99 = timings[timings.len() * 99 / 100];
        
        println!("   ✅ Complete\n");
        (avg, p50, p95, p99)
    }
    
    fn benchmark_full_cycle(detector: &mut TriangularArbitrageDetector, overhead: u64) -> (f64, u64, u64, u64) {
        let iterations = 50_000;
        let mut timings = Vec::with_capacity(iterations);
        
        println!("📊 Benchmark 3: Cycle complet (update orderbook + détection)");
        println!("   Iterations: {}", iterations);
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // Simuler une mise à jour de l'orderbook (une seule paire change)
            let price_variation = (i % 100) as i64 - 50;
            detector.pair1.apply_update(Update::Set {
                price: 31460000_i64 + (price_variation * 1000),
                quantity: 10000,
                side: Side::Ask,
            });
            
            // Détecter les opportunités
            detector.update_price_cache();
            let _opportunities = detector.detect_opportunities(12345, 1000.0);
            
            let elapsed = start.elapsed().as_nanos() as u64;
            timings.push(elapsed.saturating_sub(overhead));
        }
        
        timings.sort();
        let avg = timings.iter().sum::<u64>() as f64 / timings.len() as f64;
        let p50 = timings[timings.len() / 2];
        let p95 = timings[timings.len() * 95 / 100];
        let p99 = timings[timings.len() * 99 / 100];
        
        println!("   ✅ Complete\n");
        (avg, p50, p95, p99)
    }
    
    fn print_results(
        single: (f64, u64, u64, u64),
        with_cache: (f64, u64, u64, u64),
        full_cycle: (f64, u64, u64, u64),
    ) {
        println!("================================================================================");
        println!("  ⚡ ARBITRAGE DETECTION PERFORMANCE RESULTS");
        println!("================================================================================\n");
        
        println!("1️⃣  DÉTECTION SIMPLE (cache déjà à jour):");
        println!("    Average:  {:.2} ns", single.0);
        println!("    P50:      {} ns", single.1);
        println!("    P95:      {} ns", single.2);
        println!("    P99:      {} ns", single.3);
        Self::print_verdict(single.0);
        println!();
        
        println!("2️⃣  AVEC MISE À JOUR DU CACHE:");
        println!("    Average:  {:.2} ns", with_cache.0);
        println!("    P50:      {} ns", with_cache.1);
        println!("    P95:      {} ns", with_cache.2);
        println!("    P99:      {} ns", with_cache.3);
        Self::print_verdict(with_cache.0);
        println!();
        
        println!("3️⃣  CYCLE COMPLET (update orderbook + détection):");
        println!("    Average:  {:.2} ns", full_cycle.0);
        println!("    P50:      {} ns", full_cycle.1);
        println!("    P95:      {} ns", full_cycle.2);
        println!("    P99:      {} ns", full_cycle.3);
        Self::print_verdict(full_cycle.0);
        println!();
        
        println!("================================================================================");
        println!("📈 LATENCY ANALYSIS:");
        println!("================================================================================");
        
        let latency_us = full_cycle.0 / 1000.0;
        let latency_ms = latency_us / 1000.0;
        
        println!("   Cycle complet en microsecondes:  {:.3} μs", latency_us);
        println!("   Cycle complet en millisecondes:  {:.6} ms", latency_ms);
        println!();
        
        if latency_us < 1.0 {
            println!("   ✅ EXCELLENT: Latence sub-microseconde!");
            println!("   ✅ Très difficile à frontrun par d'autres bots");
        } else if latency_us < 10.0 {
            println!("   ✅ TRÈS BON: Latence < 10μs");
            println!("   ⚠️  Quelques bots HFT optimisés pourraient être plus rapides");
        } else if latency_us < 100.0 {
            println!("   ⚠️  BON: Latence < 100μs");
            println!("   ⚠️  De nombreux bots HFT peuvent vous devancer");
        } else {
            println!("   ❌ ATTENTION: Latence > 100μs");
            println!("   ❌ Risque élevé de se faire frontrun");
        }
        
        println!();
        println!("💡 CONTEXTE:");
        println!("   - Network latency vers exchange: ~10-50 ms (selon location)");
        println!("   - Latence calcul + réseau total: ~{:.2} ms", latency_ms + 30.0);
        println!("   - Websocket update frequency: ~100ms - 1s");
        println!();
        println!("================================================================================\n");
    }
    
    fn print_verdict(avg_ns: f64) {
        if avg_ns < 10.0 {
            println!("    🚀 EXCELLENT - Performance de niveau HFT!");
        } else if avg_ns < 100.0 {
            println!("    ✅ TRÈS BON - Compétitif pour l'arbitrage crypto");
        } else if avg_ns < 1000.0 {
            println!("    ⚠️  ACCEPTABLE - Peut être optimisé");
        } else {
            println!("    ❌ LENT - Nécessite optimisation");
        }
    }
}
