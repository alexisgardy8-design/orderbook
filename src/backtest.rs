use crate::data_loader::HistoricalUpdate;
use crate::triangular_arbitrage::{TriangularArbitrageDetector, TriangularOpportunity};
use crate::interfaces::OrderBook;

pub struct BacktestEngine {
    detector: TriangularArbitrageDetector,
    starting_capital: f64,
}

#[derive(Debug)]
pub struct BacktestResult {
    pub total_opportunities: usize,
    pub total_profit: f64,
    pub best_opportunity: Option<TriangularOpportunity>,
    pub avg_profit_per_opportunity: f64,
    pub total_updates_processed: usize,
    pub execution_time_ms: u128,
}

impl BacktestEngine {
    pub fn new(min_profit_bps: f64, starting_capital: f64) -> Self {
        Self {
            detector: TriangularArbitrageDetector::new(min_profit_bps),
            starting_capital,
        }
    }

    pub fn run(
        &mut self,
        pair1_data: Vec<HistoricalUpdate>,
        pair2_data: Vec<HistoricalUpdate>,
        pair3_data: Vec<HistoricalUpdate>,
    ) -> BacktestResult {
        let start = std::time::Instant::now();
        
        let mut all_updates: Vec<(usize, &HistoricalUpdate)> = Vec::new();
        for update in &pair1_data {
            all_updates.push((1, update));
        }
        for update in &pair2_data {
            all_updates.push((2, update));
        }
        for update in &pair3_data {
            all_updates.push((3, update));
        }
        
        all_updates.sort_by_key(|(_, update)| update.timestamp);

        let mut opportunities = Vec::new();
        let mut total_profit = 0.0;
        let mut best_opportunity: Option<TriangularOpportunity> = None;

        for (pair_num, update) in all_updates.iter() {
            match pair_num {
                1 => self.detector.pair1.apply_update(update.update.clone()),
                2 => self.detector.pair2.apply_update(update.update.clone()),
                3 => self.detector.pair3.apply_update(update.update.clone()),
                _ => {}
            }

            let detected = self.detector.detect_opportunities(
                update.timestamp,
                self.starting_capital,
            );

            for opp in detected {
                total_profit += opp.net_profit;
                
                if best_opportunity.is_none() 
                    || opp.profit_percentage > best_opportunity.as_ref().unwrap().profit_percentage 
                {
                    best_opportunity = Some(opp.clone());
                }
                
                opportunities.push(opp);
            }
        }

        let execution_time = start.elapsed().as_millis();
        let total_opportunities = opportunities.len();
        let avg_profit = if total_opportunities > 0 {
            total_profit / total_opportunities as f64
        } else {
            0.0
        };

        BacktestResult {
            total_opportunities,
            total_profit,
            best_opportunity,
            avg_profit_per_opportunity: avg_profit,
            total_updates_processed: all_updates.len(),
            execution_time_ms: execution_time,
        }
    }
}
