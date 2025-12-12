use crate::interfaces::{Price, Quantity, Side, Update};
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct HistoricalUpdate {
    pub timestamp: u64,
    pub symbol: String,
    pub update: Update,
}

pub struct DataLoader;

impl DataLoader {
    pub fn load_from_csv<P: AsRef<Path>>(
        path: P,
        symbol: &str,
    ) -> Result<Vec<HistoricalUpdate>, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut updates = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            if line_num == 0 {
                continue;
            }

            let line = line?;
            let fields: Vec<&str> = line.split(',').collect();

            if fields.len() < 5 {
                continue;
            }

            let timestamp = fields[0].parse::<u64>()?;
            let price_float = fields[1].parse::<f64>()?;
            let quantity_float = fields[2].parse::<f64>()?;
            let side_str = fields[3];

            let price = (price_float * 10000.0) as Price;
            let quantity = (quantity_float * 1000.0) as Quantity;
            let side = match side_str {
                "bid" | "BID" | "buy" | "BUY" => Side::Bid,
                "ask" | "ASK" | "sell" | "SELL" => Side::Ask,
                _ => continue,
            };

            updates.push(HistoricalUpdate {
                timestamp,
                symbol: symbol.to_string(),
                update: Update::Set {
                    price,
                    quantity,
                    side,
                },
            });
        }

        updates.sort_by_key(|u| u.timestamp);
        Ok(updates)
    }

    pub fn generate_realistic_arbitrage_data(
        symbol: &str,
        num_updates: usize,
        base_price: f64,
        volatility: f64,
    ) -> Vec<HistoricalUpdate> {
        let mut updates = Vec::with_capacity(num_updates * 2);
        let mut timestamp = 1000000000;
        
        for i in 0..num_updates {
            let time_factor = i as f64 / 100.0;
            let price_wave = (time_factor * 0.1).sin() * volatility;
            let price_drift = (time_factor * 0.05).cos() * volatility * 0.5;
            let price_variation = base_price * (1.0 + price_wave + price_drift);
            
            let price = (price_variation * 10000.0) as Price;
            let quantity = (100.0 + (i % 100) as f64 * 5.0) as Quantity;
            
            let spread_bps = 20 + (i % 30) as i64;
            let bid_price = price - spread_bps;
            let ask_price = price + spread_bps;
            
            updates.push(HistoricalUpdate {
                timestamp,
                symbol: symbol.to_string(),
                update: Update::Set {
                    price: bid_price,
                    quantity,
                    side: Side::Bid,
                },
            });
            
            updates.push(HistoricalUpdate {
                timestamp,
                symbol: symbol.to_string(),
                update: Update::Set {
                    price: ask_price,
                    quantity,
                    side: Side::Ask,
                },
            });
            
            timestamp += 50 + (i % 20) as u64;
        }
        
        updates
    }

    pub fn generate_sample_data(
        symbol: &str,
        num_updates: usize,
        base_price: f64,
    ) -> Vec<HistoricalUpdate> {
        let mut updates = Vec::with_capacity(num_updates);
        let mut timestamp = 1000000000;

        for i in 0..num_updates {
            let price_variation = ((i as f64 * 0.01).sin() * 0.02 + 1.0) * base_price;
            let price = (price_variation * 10000.0) as Price;
            let quantity = (100.0 + (i % 50) as f64 * 10.0) as Quantity;
            
            let bid_offset = (i % 20) as i64;
            let ask_offset = 100 + (i % 30) as i64;

            updates.push(HistoricalUpdate {
                timestamp,
                symbol: symbol.to_string(),
                update: Update::Set {
                    price: price - bid_offset,
                    quantity,
                    side: Side::Bid,
                },
            });

            updates.push(HistoricalUpdate {
                timestamp,
                symbol: symbol.to_string(),
                update: Update::Set {
                    price: price + ask_offset,
                    quantity,
                    side: Side::Ask,
                },
            });

            timestamp += 100 + (i % 10) as u64;
        }

        updates
    }
}
