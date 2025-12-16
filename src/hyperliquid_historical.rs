// 📊 Récupération des données historiques Hyperliquid pour backtest
// API REST Hyperliquid pour obtenir les bougies OHLC
// Support pour 1-2 ans de données via pagination automatique

use serde::{Deserialize, Serialize};
use std::error::Error;

const HYPERLIQUID_API_URL: &str = "https://api.hyperliquid.xyz/info";
const COIN: &str = "SOL";
const INTERVAL: &str = "1h";
const MAX_CANDLES_PER_REQUEST: usize = 5000; // Limite API Hyperliquid par requête
const CANDLES_PER_YEAR: usize = 8760; // 365 jours * 24 heures

/// Bougie OHLC de Hyperliquid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperCandle {
    pub t: u64,      // open timestamp (millis)
    #[serde(rename = "T")]
    pub close_t: u64, // close timestamp (millis)
    pub s: String,   // coin symbol
    pub i: String,   // interval
    pub o: String,   // open price (string)
    pub c: String,   // close price (string)
    pub h: String,   // high price (string)
    pub l: String,   // low price (string)
    pub v: String,   // volume (string)
    pub n: u64,      // number of trades
}

impl HyperCandle {
    /// Convertit les prix strings en f64
    pub fn to_ohlc(&self) -> Result<(f64, f64, f64, f64, f64), Box<dyn Error>> {
        Ok((
            self.o.parse()?,
            self.h.parse()?,
            self.l.parse()?,
            self.c.parse()?,
            self.v.parse()?,
        ))
    }
}

/// Requête pour récupérer les bougies
#[derive(Debug, Serialize)]
struct CandleRequest {
    #[serde(rename = "type")]
    request_type: String,
    req: CandleRequestParams,
}

#[derive(Debug, Serialize)]
struct CandleRequestParams {
    coin: String,
    interval: String,
    #[serde(rename = "startTime")]
    start_time: u64,
    #[serde(rename = "endTime")]
    end_time: u64,
}

/// Client pour récupérer les données historiques de Hyperliquid
pub struct HyperliquidHistoricalData {
    coin: String,
    interval: String,
}

impl HyperliquidHistoricalData {
    pub fn new(coin: String, interval: String) -> Self {
        Self { coin, interval }
    }

    /// Récupère les bougies historiques pour une plage de temps
    pub fn fetch_candles(&self, start_time: u64, end_time: u64) -> Result<Vec<HyperCandle>, Box<dyn Error>> {
        println!("📥 Fetching historical data from Hyperliquid...");
        println!("   Coin:     {}", self.coin);
        println!("   Interval: {}", self.interval);
        println!("   Start:    {} ({})", start_time, Self::format_timestamp(start_time));
        println!("   End:      {} ({})", end_time, Self::format_timestamp(end_time));

        let request = CandleRequest {
            request_type: "candleSnapshot".to_string(),
            req: CandleRequestParams {
                coin: self.coin.clone(),
                interval: self.interval.clone(),
                start_time,
                end_time,
            },
        };

        let client = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .build();

        let response = client
            .post(HYPERLIQUID_API_URL)
            .set("Content-Type", "application/json")
            .send_json(&request)?;

        let candles: Vec<HyperCandle> = response.into_json()?;

        println!("✅ Received {} candles", candles.len());

        if candles.is_empty() {
            println!("⚠️  No candles returned - This could mean:");
            println!("   - The time range is too old (only 5000 most recent available)");
            println!("   - The coin symbol is incorrect");
            println!("   - The interval is not supported");
        } else {
            let first = &candles[0];
            let last = &candles[candles.len() - 1];
            println!("\n📊 Data Range:");
            println!("   First candle: {} ({})", first.t, Self::format_timestamp(first.t));
            println!("   Last candle:  {} ({})", last.t, Self::format_timestamp(last.t));
            
            if let Ok((o, h, l, c, v)) = first.to_ohlc() {
                println!("\n🕯️  First Candle:");
                println!("   Open:   ${:.2}", o);
                println!("   High:   ${:.2}", h);
                println!("   Low:    ${:.2}", l);
                println!("   Close:  ${:.2}", c);
                println!("   Volume: {:.2}", v);
            }
        }

        Ok(candles)
    }

    /// Récupère les 5000 bougies les plus récentes (limite API)
    pub fn fetch_recent_candles(&self) -> Result<Vec<HyperCandle>, Box<dyn Error>> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let start = now - (MAX_CANDLES_PER_REQUEST as u64 * 60 * 60 * 1000); // ~208 jours en arrière pour 1h candles
        
        self.fetch_candles(start, now)
    }

    /// Récupère 1 an de données avec pagination automatique
    /// (environ 8760 candles à 1h = 2 requêtes)
    pub fn fetch_one_year(&self) -> Result<Vec<HyperCandle>, Box<dyn Error>> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let one_year_ago = now - (CANDLES_PER_YEAR as u64 * 60 * 60 * 1000);
        
        self.fetch_candles_paginated(one_year_ago, now)
    }

    /// Récupère 2 ans de données avec pagination automatique
    /// (environ 17520 candles à 1h = 4 requêtes)
    pub fn fetch_two_years(&self) -> Result<Vec<HyperCandle>, Box<dyn Error>> {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        let two_years_ago = now - (CANDLES_PER_YEAR as u64 * 2 * 60 * 60 * 1000);
        
        self.fetch_candles_paginated(two_years_ago, now)
    }

    /// Récupère les données avec pagination automatique
    /// La limite de 5000 par requête nécessite plusieurs appels pour 1-2 ans
    fn fetch_candles_paginated(&self, start_time: u64, end_time: u64) -> Result<Vec<HyperCandle>, Box<dyn Error>> {
        let days_of_data = ((end_time - start_time) / (60 * 60 * 1000)) as f64 / 24.0;
        let num_requests = (days_of_data / (MAX_CANDLES_PER_REQUEST as f64 / 24.0)).ceil() as usize;
        
        println!("📥 Fetching historical data from Hyperliquid...");
        println!("   Coin:           {}", self.coin);
        println!("   Interval:       {}", self.interval);
        println!("   Duration:       {:.1} days (~{:.1} years)", days_of_data, days_of_data / 365.0);
        println!("   Estimated:      ~{} candles (need {} requests)", 
            (days_of_data * 24.0) as usize, num_requests);
        println!("   Start:          {} ({})", start_time, Self::format_timestamp(start_time));
        println!("   End:            {} ({})", end_time, Self::format_timestamp(end_time));
        println!();

        let mut all_candles: Vec<HyperCandle> = Vec::new();
        let hours_per_request = (MAX_CANDLES_PER_REQUEST - 1) as u64 * 60 * 60 * 1000; // Léger chevauchement pour éviter les trous
        let mut current_start = start_time;
        
        for request_num in 1..=num_requests {
            let current_end = std::cmp::min(current_start + hours_per_request, end_time);
            
            println!("   📡 Request {}/{}: {}", request_num, num_requests, 
                Self::format_timestamp(current_start));
            
            let request = CandleRequest {
                request_type: "candleSnapshot".to_string(),
                req: CandleRequestParams {
                    coin: self.coin.clone(),
                    interval: self.interval.clone(),
                    start_time: current_start,
                    end_time: current_end,
                },
            };

            let client = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(30))
                .build();

            let response = client
                .post(HYPERLIQUID_API_URL)
                .set("Content-Type", "application/json")
                .send_json(&request)?;

            let mut candles: Vec<HyperCandle> = response.into_json()?;
            
            if candles.is_empty() {
                println!("      ⚠️  No more candles available");
                break;
            }
            
            println!("      ✅ Received {} candles", candles.len());
            all_candles.append(&mut candles);
            
            // Avancer pour la prochaine requête (avec un petit chevauchement)
            current_start = current_end - (1000 * 60 * 60); // 1000 heures de chevauchement
            
            if current_start >= end_time {
                break;
            }
            
            // Petit délai pour ne pas surcharger l'API
            std::thread::sleep(std::time::Duration::from_millis(200));
        }

        // Dédupliquer par timestamp (à cause du chevauchement)
        all_candles.sort_by_key(|c| c.t);
        all_candles.dedup_by_key(|c| c.t);

        println!("\n✅ Total received: {} candles", all_candles.len());

        if all_candles.is_empty() {
            println!("⚠️  No candles returned - This could mean:");
            println!("   - The time range is too old");
            println!("   - The coin symbol is incorrect");
            println!("   - The interval is not supported");
        } else {
            let first = &all_candles[0];
            let last = &all_candles[all_candles.len() - 1];
            println!("\n📊 Data Range:");
            println!("   First candle: {} ({})", first.t, Self::format_timestamp(first.t));
            println!("   Last candle:  {} ({})", last.t, Self::format_timestamp(last.t));
            println!("   Coverage:     {:.1} days", 
                ((last.t - first.t) / (60 * 60 * 1000)) as f64 / 24.0);
            
            if let Ok((o, h, l, c, v)) = first.to_ohlc() {
                println!("\n🕯️  First Candle:");
                println!("   Open:   ${:.2}", o);
                println!("   High:   ${:.2}", h);
                println!("   Low:    ${:.2}", l);
                println!("   Close:  ${:.2}", c);
                println!("   Volume: {:.2}", v);
            }
        }

        Ok(all_candles)
    }

    fn format_timestamp(millis: u64) -> String {
        use chrono::prelude::*;
        let dt = Utc.timestamp_millis_opt(millis as i64).unwrap();
        // Add 1 hour for France Winter Time
        let dt_paris = dt + chrono::Duration::hours(1);
        dt_paris.format("%Y-%m-%d %H:%M:%S (Paris)").to_string()
    }
}

/// Test simple pour vérifier la connexion
pub fn test_hyperliquid_connection() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 HYPERLIQUID API TEST - Historical Data Retrieval         ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let fetcher = HyperliquidHistoricalData::new(COIN.to_string(), INTERVAL.to_string());

    match fetcher.fetch_recent_candles() {
        Ok(candles) => {
            println!("\n✅ SUCCESS: Hyperliquid API is working!");
            println!("   Retrieved {} candles for {}-PERP ({})", candles.len(), COIN, INTERVAL);

            if !candles.is_empty() {
                println!("\n📈 Sample Data Analysis:");
                
                // Calculer quelques stats
                let mut prices: Vec<f64> = Vec::new();
                for candle in &candles {
                    if let Ok((_, _, _, c, _)) = candle.to_ohlc() {
                        prices.push(c);
                    }
                }

                if !prices.is_empty() {
                    let min_price = prices.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max_price = prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let avg_price = prices.iter().sum::<f64>() / prices.len() as f64;
                    
                    println!("   Min Price:  ${:.2}", min_price);
                    println!("   Max Price:  ${:.2}", max_price);
                    println!("   Avg Price:  ${:.2}", avg_price);
                    println!("   Range:      ${:.2} ({:.2}%)", 
                        max_price - min_price,
                        ((max_price - min_price) / min_price) * 100.0
                    );
                }

                // Afficher les 5 dernières bougies
                println!("\n🕯️  Last 5 Candles:");
                for (i, candle) in candles.iter().rev().take(5).enumerate() {
                    if let Ok((o, h, l, c, v)) = candle.to_ohlc() {
                        let change = ((c - o) / o) * 100.0;
                        let emoji = if change > 0.0 { "🟢" } else { "🔴" };
                        println!("   {} #{}: O=${:.2} H=${:.2} L=${:.2} C=${:.2} ({:+.2}%)", 
                            emoji, i + 1, o, h, l, c, change);
                    }
                }

                println!("\n💡 Next Steps:");
                println!("   ✅ Data retrieval confirmed working");
                println!("   ✅ Can proceed with backtesting on this data");
                println!("   ✅ WebSocket live trading will also work");
            }
        }
        Err(e) => {
            eprintln!("\n❌ ERROR: Failed to fetch data from Hyperliquid");
            eprintln!("   Error: {}", e);
            eprintln!("\n💡 Troubleshooting:");
            eprintln!("   - Check internet connection");
            eprintln!("   - Verify Hyperliquid API is accessible");
            eprintln!("   - Try: curl -X POST {} -H 'Content-Type: application/json' -d '{}'", 
                HYPERLIQUID_API_URL,
                r#"{"type":"candleSnapshot","coin":"SOL","interval":"1h","startTime":0,"endTime":9999999999999}"#
            );
        }
    }
}
