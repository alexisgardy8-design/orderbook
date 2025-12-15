use reqwest::Client;
use serde_json::json;
use std::env;

#[derive(Clone)]
pub struct TelegramBot {
    token: String,
    chat_id: String,
    client: Client,
}

impl TelegramBot {
    pub fn new() -> Option<Self> {
        let token = env::var("TELEGRAM_BOT_TOKEN").ok()?;
        let chat_id = env::var("TELEGRAM_CHAT_ID").ok()?;
        
        if token.is_empty() || chat_id.is_empty() {
            return None;
        }

        Some(Self {
            token,
            chat_id,
            client: Client::new(),
        })
    }

    pub async fn send_message(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let params = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown" // Allows bold, italic, etc.
        });

        let response = self.client.post(&url)
            .json(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Telegram API Error: {}", error_text).into());
        }
            
        Ok(())
    }
}
