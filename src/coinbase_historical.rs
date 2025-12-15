// 📊 Récupération des données historiques Coinbase pour SOL-USD
// API REST Coinbase pour obtenir les bougies OHLC historiques
// Note: SOL-USDC perpétuel non disponible sur REST API, utilisation de SOL-USD spot

use serde::{Deserialize, Serialize};
use std::error::Error;
use std::thread;
use std::time::Duration;

const COINBASE_API_URL: &str = "https://api.exchange.coinbase.com";
const PRODUCT_ID: &str = "SOL-USD";
const GRANULARITY: u32 = 3600; // 1 heure en secondes
const MAX_CANDLES_PER_REQUEST: usize = 300;

/// Bougie OHLC (Open, High, Low, Close)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub timestamp: u64,      // Unix timestamp (secondes)
    pub open: f64,          // Prix d'ouverture
    pub high: f64,          // Prix le plus haut
    pub low: f64,           // Prix le plus bas
    pub close: f64,         // Prix de clôture
    pub volume: f64,        // Volume de trading
}

/// Réponse de l'API Coinbase pour les bougies
/// Format: [[time, low, high, open, close, volume], ...]
type CoinbaseCandleResponse = Vec<[serde_json::Value; 6]>;

/// Client pour récupérer les données historiques de Coinbase
pub struct CoinbaseHistoricalData {
    product_id: String,
    granularity: u32,
}

impl CoinbaseHistoricalData {
    pub fn new() -> Self {
        Self {
            product_id: PRODUCT_ID.to_string(),
            granularity: GRANULARITY,
        }
    }

    /// Récupère les bougies historiques sur une période donnée
    /// 
    /// # Arguments
    /// * `start` - Timestamp Unix de début (secondes)
    /// * `end` - Timestamp Unix de fin (secondes)
    pub fn fetch_candles(&self, start: u64, end: u64) -> Result<Vec<Candle>, Box<dyn Error>> {
        let mut all_candles = Vec::new();
        let mut current_start = start;
        
        println!("📥 Fetching historical data for {} from Coinbase...", self.product_id);
        println!("   Period: {} to {}", 
            format_timestamp(start), 
            format_timestamp(end));
        
        let total_hours = (end - start) / 3600;
        let total_requests = (total_hours as usize + MAX_CANDLES_PER_REQUEST - 1) / MAX_CANDLES_PER_REQUEST;
        let mut request_count = 0;
        
        while current_start < end {
            request_count += 1;
            
            // Calculer la fin de cette requête (max 300 bougies)
            let batch_end = std::cmp::min(
                current_start + (MAX_CANDLES_PER_REQUEST as u64 * self.granularity as u64),
                end
            );
            
            println!("   Request {}/{}: Fetching {} candles...", 
                request_count, 
                total_requests,
                (batch_end - current_start) / self.granularity as u64);
            
            // Requête HTTP
            let url = format!(
                "{}/products/{}/candles?start={}&end={}&granularity={}",
                COINBASE_API_URL,
                self.product_id,
                current_start,
                batch_end,
                self.granularity
            );
            
            match self.fetch_batch(&url) {
                Ok(mut candles) => {
                    println!("   ✅ Received {} candles", candles.len());
                    all_candles.append(&mut candles);
                }
                Err(e) => {
                    eprintln!("   ❌ Error fetching batch: {}", e);
                    eprintln!("   Retrying in 2 seconds...");
                    thread::sleep(Duration::from_secs(2));
                    continue; // Retry
                }
            }
            
            current_start = batch_end;
            
            // Rate limiting: pause entre les requêtes
            if current_start < end {
                thread::sleep(Duration::from_millis(500));
            }
        }
        
        // Trier par timestamp (ordre chronologique)
        all_candles.sort_by_key(|c| c.timestamp);
        
        println!("\n✅ Total candles fetched: {}", all_candles.len());
        println!("   First candle: {} (${:.2})", 
            format_timestamp(all_candles.first().map(|c| c.timestamp).unwrap_or(0)),
            all_candles.first().map(|c| c.close).unwrap_or(0.0));
        println!("   Last candle:  {} (${:.2})\n", 
            format_timestamp(all_candles.last().map(|c| c.timestamp).unwrap_or(0)),
            all_candles.last().map(|c| c.close).unwrap_or(0.0));
        
        Ok(all_candles)
    }
    
    /// Récupère un lot de bougies via HTTP
    fn fetch_batch(&self, url: &str) -> Result<Vec<Candle>, Box<dyn Error>> {
        // Utiliser ureq pour une requête HTTP simple (sync)
        let response = ureq::get(url)
            .set("User-Agent", "rust-orderbook-bot/0.3.0")
            .call()?;
        
        let body = response.into_string()?;
        let raw_candles: CoinbaseCandleResponse = serde_json::from_str(&body)?;
        
        let candles = raw_candles
            .into_iter()
            .filter_map(|candle_data| {
                // Format Coinbase: [time, low, high, open, close, volume]
                let timestamp = candle_data[0].as_u64()?;
                let low = candle_data[1].as_f64()?;
                let high = candle_data[2].as_f64()?;
                let open = candle_data[3].as_f64()?;
                let close = candle_data[4].as_f64()?;
                let volume = candle_data[5].as_f64()?;
                
                Some(Candle {
                    timestamp,
                    open,
                    high,
                    low,
                    close,
                    volume,
                })
            })
            .collect();
        
        Ok(candles)
    }
    
    /// Récupère les données sur 1 an
    pub fn fetch_one_year(&self) -> Result<Vec<Candle>, Box<dyn Error>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let one_year_ago = now - (365 * 24 * 3600);
        
        self.fetch_candles(one_year_ago, now)
    }
    
    /// Récupère les données sur N jours
    pub fn fetch_last_n_days(&self, days: u64) -> Result<Vec<Candle>, Box<dyn Error>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let start = now - (days * 24 * 3600);
        
        self.fetch_candles(start, now)
    }
}

/// Formate un timestamp Unix en date lisible
fn format_timestamp(timestamp: u64) -> String {
    use chrono::{DateTime, Utc, TimeZone};
    let dt = Utc.timestamp_opt(timestamp as i64, 0).unwrap();
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// Sauvegarde les bougies dans un fichier CSV
pub fn save_candles_to_csv(candles: &[Candle], filename: &str) -> Result<(), Box<dyn Error>> {
    use std::fs::File;
    use std::io::Write;
    
    let mut file = File::create(filename)?;
    
    // Header
    writeln!(file, "timestamp,datetime,open,high,low,close,volume")?;
    
    // Data
    for candle in candles {
        writeln!(
            file,
            "{},{},{},{},{},{},{}",
            candle.timestamp,
            format_timestamp(candle.timestamp),
            candle.open,
            candle.high,
            candle.low,
            candle.close,
            candle.volume
        )?;
    }
    
    println!("💾 Saved {} candles to {}", candles.len(), filename);
    Ok(())
}

/// Charge les bougies depuis un fichier CSV
pub fn load_candles_from_csv(filename: &str) -> Result<Vec<Candle>, Box<dyn Error>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let mut candles = Vec::new();
    
    for (i, line) in reader.lines().enumerate() {
        if i == 0 {
            continue; // Skip header
        }
        
        let line = line?;
        let parts: Vec<&str> = line.split(',').collect();
        
        if parts.len() < 7 {
            continue;
        }
        
        let candle = Candle {
            timestamp: parts[0].parse()?,
            open: parts[2].parse()?,
            high: parts[3].parse()?,
            low: parts[4].parse()?,
            close: parts[5].parse()?,
            volume: parts[6].parse()?,
        };
        
        candles.push(candle);
    }
    
    println!("📂 Loaded {} candles from {}", candles.len(), filename);
    Ok(candles)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // Ignore par défaut car nécessite une connexion internet
    fn test_fetch_last_7_days() {
        let client = CoinbaseHistoricalData::new();
        let result = client.fetch_last_n_days(7);
        
        assert!(result.is_ok());
        let candles = result.unwrap();
        assert!(candles.len() > 0);
        println!("Fetched {} candles for last 7 days", candles.len());
    }
}
