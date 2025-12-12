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
            pair1: OrderBookImpl::new(),
            pair2: OrderBookImpl::new(),
            pair3: OrderBookImpl::new(),
            trading_fee: 0.001,
            min_profit_bps,
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
        self.cached_price1_ask = self.pair1.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price1_bid = self.pair1.get_best_bid().unwrap_or(0) as f64 / 10000.0;
        self.cached_price2_ask = self.pair2.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price2_bid = self.pair2.get_best_bid().unwrap_or(0) as f64 / 10000.0;
        self.cached_price3_ask = self.pair3.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price3_bid = self.pair3.get_best_bid().unwrap_or(0) as f64 / 10000.0;
    }
    
    #[inline(always)]
    fn update_price_cache_with_refs(&mut self, ob1: &OrderBookImpl, ob2: &OrderBookImpl, ob3: &OrderBookImpl) {
        self.cached_price1_ask = ob1.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price1_bid = ob1.get_best_bid().unwrap_or(0) as f64 / 10000.0;
        self.cached_price2_ask = ob2.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price2_bid = ob2.get_best_bid().unwrap_or(0) as f64 / 10000.0;
        self.cached_price3_ask = ob3.get_best_ask().unwrap_or(0) as f64 / 10000.0;
        self.cached_price3_bid = ob3.get_best_bid().unwrap_or(0) as f64 / 10000.0;
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

        let fee_multiplier = 1.0 - self.trading_fee;
        let amount1 = starting_amount / self.cached_price1_ask * fee_multiplier;
        let amount2 = amount1 * self.cached_price2_bid * fee_multiplier;
        let final_amount = amount2 * self.cached_price3_bid * fee_multiplier;

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

        let fee_multiplier = 1.0 - self.trading_fee;
        let amount1 = starting_amount / self.cached_price3_ask * fee_multiplier;
        let amount2 = amount1 / self.cached_price2_ask * fee_multiplier;
        let final_amount = amount2 * self.cached_price1_bid * fee_multiplier;

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
