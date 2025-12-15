use crate::interfaces::OrderBook;
use crate::orderbook::OrderBookImpl;

#[derive(Debug, Clone)]
pub struct TriangularOpportunity {
    pub timestamp: u64,
    pub path: ArbitragePath,
    pub profit_percentage: f64,
    pub input_amount: f64,
    pub expected_output: f64,
    pub net_profit: f64,
}

#[derive(Debug, Clone)]
pub enum ArbitragePath {
    Forward,
    Reverse,
}

pub struct TriangularArbitrageDetector {
    pub pair1: OrderBookImpl,
    pub pair2: OrderBookImpl,
    pub pair3: OrderBookImpl,
    
    trading_fee: f64,
    min_profit_bps: f64,
    
    // Facteurs de conversion pour chaque paire
    pair1_divisor: f64,  // ETH-USDC: 10000 (4 décimales)
    pair2_divisor: f64,  // BTC-USDC: 10000 (4 décimales)
    pair3_divisor: f64,  // ETH-BTC: 100000000 (8 décimales pour plus de précision)
    
    cached_price1_ask: f64,
    cached_price1_bid: f64,
    cached_price2_ask: f64,
    cached_price2_bid: f64,
    cached_price3_ask: f64,
    cached_price3_bid: f64,
}

impl TriangularArbitrageDetector {
    pub fn new(min_profit_bps: f64) -> Self {
        Self {
            // pair1: ETH-USDC ($2000 to $5000) - currently ~$3146
            // Utilise 10000 (4 décimales) : 3146.00 → 31460000
            pair1: OrderBookImpl::with_range(20_000_000, 50_000_000),
            // pair2: BTC-USDC ($70k to $120k) - currently ~$89,903.62
            // Utilise 10000 (4 décimales) : 89903.62 → 899036200
            pair2: OrderBookImpl::with_range(700_000_000, 1_200_000_000),
            // pair3: ETH-BTC (0.02 to 0.06 BTC) - currently ~0.03499
            // Utilise 100000000 (8 décimales) pour plus de précision : 0.03499 → 3499000
            pair3: OrderBookImpl::with_range(2_000_000, 6_000_000),
            trading_fee: 0.001,
            min_profit_bps,
            pair1_divisor: 10000.0,
            pair2_divisor: 10000.0,
            pair3_divisor: 100000000.0,
            cached_price1_ask: 0.0,
            cached_price1_bid: 0.0,
            cached_price2_ask: 0.0,
            cached_price2_bid: 0.0,
            cached_price3_ask: 0.0,
            cached_price3_bid: 0.0,
        }
    }

    #[inline(always)]
    pub fn update_price_cache(&mut self) {
        self.cached_price1_ask = self.pair1.get_best_ask().unwrap_or(0) as f64 / self.pair1_divisor;
        self.cached_price1_bid = self.pair1.get_best_bid().unwrap_or(0) as f64 / self.pair1_divisor;
        self.cached_price2_ask = self.pair2.get_best_ask().unwrap_or(0) as f64 / self.pair2_divisor;
        self.cached_price2_bid = self.pair2.get_best_bid().unwrap_or(0) as f64 / self.pair2_divisor;
        self.cached_price3_ask = self.pair3.get_best_ask().unwrap_or(0) as f64 / self.pair3_divisor;
        self.cached_price3_bid = self.pair3.get_best_bid().unwrap_or(0) as f64 / self.pair3_divisor;
    }
    
    #[inline(always)]
    fn update_price_cache_with_refs(&mut self, ob1: &OrderBookImpl, ob2: &OrderBookImpl, ob3: &OrderBookImpl) {
        self.cached_price1_ask = ob1.get_best_ask().unwrap_or(0) as f64 / self.pair1_divisor;
        self.cached_price1_bid = ob1.get_best_bid().unwrap_or(0) as f64 / self.pair1_divisor;
        self.cached_price2_ask = ob2.get_best_ask().unwrap_or(0) as f64 / self.pair2_divisor;
        self.cached_price2_bid = ob2.get_best_bid().unwrap_or(0) as f64 / self.pair2_divisor;
        self.cached_price3_ask = ob3.get_best_ask().unwrap_or(0) as f64 / self.pair3_divisor;
        self.cached_price3_bid = ob3.get_best_bid().unwrap_or(0) as f64 / self.pair3_divisor;
    }

    #[inline(always)]
    pub fn detect_opportunities(
        &mut self,
        timestamp: u64,
        starting_amount: f64,
    ) -> Vec<TriangularOpportunity> {
        self.update_price_cache();
        
        let mut opportunities = Vec::with_capacity(2);

        if let Some(opp) = self.check_forward_path_fast(timestamp, starting_amount) {
            opportunities.push(opp);
        }

        if let Some(opp) = self.check_reverse_path_fast(timestamp, starting_amount) {
            opportunities.push(opp);
        }

        opportunities
    }

    #[inline(always)]
    pub fn detect_opportunities_with_refs(
        &mut self,
        ob1: &OrderBookImpl,
        ob2: &OrderBookImpl,
        ob3: &OrderBookImpl,
        timestamp: u64,
        starting_amount: f64,
    ) -> Vec<TriangularOpportunity> {
        self.update_price_cache_with_refs(ob1, ob2, ob3);
        
        let mut opportunities = Vec::with_capacity(2);

        if let Some(opp) = self.check_forward_path_fast(timestamp, starting_amount) {
            opportunities.push(opp);
        }

        if let Some(opp) = self.check_reverse_path_fast(timestamp, starting_amount) {
            opportunities.push(opp);
        }

        opportunities
    }

    #[inline(always)]
    fn check_forward_path_fast(
        &self,
        timestamp: u64,
        starting_amount: f64,
    ) -> Option<TriangularOpportunity> {
        if self.cached_price1_ask == 0.0 || self.cached_price2_bid == 0.0 || self.cached_price3_bid == 0.0 {
            return None;
        }

        // Forward: USDC → ETH → BTC → USDC
        // 1. Buy ETH with USDC (using pair1 ETH-USDC ask price)
        let fee_multiplier = 1.0 - self.trading_fee;
        let eth_amount = (starting_amount / self.cached_price1_ask) * fee_multiplier;
        // 2. Sell ETH for BTC (using pair3 ETH-BTC bid price)
        let btc_amount = (eth_amount * self.cached_price3_bid) * fee_multiplier;
        // 3. Sell BTC for USDC (using pair2 BTC-USDC bid price)
        let final_amount = (btc_amount * self.cached_price2_bid) * fee_multiplier;

        let profit = final_amount - starting_amount;
        let profit_bps = (profit / starting_amount) * 10000.0;

        if profit_bps >= self.min_profit_bps {
            Some(TriangularOpportunity {
                timestamp,
                path: ArbitragePath::Forward,
                profit_percentage: profit_bps / 100.0,
                input_amount: starting_amount,
                expected_output: final_amount,
                net_profit: profit,
            })
        } else {
            None
        }
    }

    #[inline(always)]
    fn check_reverse_path_fast(
        &self,
        timestamp: u64,
        starting_amount: f64,
    ) -> Option<TriangularOpportunity> {
        if self.cached_price1_bid == 0.0 || self.cached_price2_ask == 0.0 || self.cached_price3_ask == 0.0 {
            return None;
        }

        // Reverse: USDC → BTC → ETH → USDC
        // 1. Buy BTC with USDC (using pair2 BTC-USDC ask price)
        let fee_multiplier = 1.0 - self.trading_fee;
        let btc_amount = (starting_amount / self.cached_price2_ask) * fee_multiplier;
        // 2. Buy ETH with BTC (using pair3 ETH-BTC ask price)
        let eth_amount = (btc_amount / self.cached_price3_ask) * fee_multiplier;
        // 3. Sell ETH for USDC (using pair1 ETH-USDC bid price)
        let final_amount = (eth_amount * self.cached_price1_bid) * fee_multiplier;

        let profit = final_amount - starting_amount;
        let profit_bps = (profit / starting_amount) * 10000.0;

        if profit_bps >= self.min_profit_bps {
            Some(TriangularOpportunity {
                timestamp,
                path: ArbitragePath::Reverse,
                profit_percentage: profit_bps / 100.0,
                input_amount: starting_amount,
                expected_output: final_amount,
                net_profit: profit,
            })
        } else {
            None
        }
    }
}
