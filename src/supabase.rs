use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::env;
use std::error::Error;
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbLog {
    pub level: String,
    pub message: String,
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPosition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>, // Supabase ID
    pub coin: String,
    pub side: String, // "LONG" or "SHORT"
    pub entry_price: f64,
    pub size: f64,
    pub status: String, // "OPEN", "CLOSED"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pnl: Option<f64>,
}

#[derive(Clone)]
pub struct SupabaseClient {
    client: Client,
    url: String,
    key: String,
}

impl SupabaseClient {
    pub fn new() -> Option<Self> {
        let url = env::var("SUPABASE_URL").ok()?;
        let key = env::var("SUPABASE_KEY").ok()?;
        
        Some(Self {
            client: Client::new(),
            url,
            key,
        })
    }

    pub async fn log(&self, level: &str, message: &str, context: Option<&str>) -> Result<(), Box<dyn Error>> {
        let url = format!("{}/rest/v1/bot_logs", self.url);
        
        // Adjust time to France (UTC+1) manually since server is UTC
        let now = Utc::now();
        let adjusted_time = now + Duration::hours(1);

        let log_entry = DbLog {
            level: level.to_string(),
            message: message.to_string(),
            context: context.map(|s| s.to_string()),
            created_at: Some(adjusted_time),
        };

        let response = self.client.post(&url)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .json(&log_entry)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            eprintln!("Failed to log to Supabase ({}): {}", status, text);
            // We don't return error here to avoid breaking the main loop just because logging failed
        }

        Ok(())
    }

    pub async fn fetch_last_logs(&self, limit: usize) -> Result<Vec<DbLog>, Box<dyn Error>> {
        let url = format!("{}/rest/v1/bot_logs?select=*&order=created_at.desc&limit={}", self.url, limit);
        
        let response = self.client.get(&url)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Accept", "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Supabase error: {}", error_text).into());
        }

        let logs: Vec<DbLog> = response.json().await?;
        Ok(logs)
    }

    pub async fn fetch_open_positions(&self) -> Result<Vec<DbPosition>, Box<dyn Error>> {
        let url = format!("{}/rest/v1/positions?status=eq.OPEN&select=*", self.url);
        
        let response = self.client.get(&url)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Accept", "application/json")
            .send()
            .await?;

        // Check status code
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Supabase error: {}", error_text).into());
        }

        let positions: Vec<DbPosition> = response.json().await?;
        Ok(positions)
    }

    pub async fn save_position(&self, position: &DbPosition) -> Result<i64, Box<dyn Error>> {
        let url = format!("{}/rest/v1/positions", self.url);
        
        // We need to get the ID back, so we use Prefer: return=representation
        let response = self.client.post(&url)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=representation")
            .json(position)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("Supabase error ({}): {}", status, text).into());
        }

        let created: Vec<DbPosition> = response.json().await?;
        if let Some(p) = created.first() {
            Ok(p.id.unwrap_or(0))
        } else {
            Err("Failed to create position".into())
        }
    }

    pub async fn update_position(&self, id: i64, update: &DbPosition) -> Result<(), Box<dyn Error>> {
        let url = format!("{}/rest/v1/positions?id=eq.{}", self.url, id);
        
        let response = self.client.patch(&url)
            .header("apikey", &self.key)
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .json(update)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(format!("Supabase update error ({}): {}", status, text).into());
        }

        Ok(())
    }
}
