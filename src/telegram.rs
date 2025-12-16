use reqwest::Client;
use serde_json::json;
use std::env;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tokio::sync::Mutex;
use crate::position_manager::PositionManager;

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
            "parse_mode": "Markdown"
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

    pub async fn send_control_keyboard(&self, is_running: bool) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        
        let status_text = if is_running { "🟢 Bot is RUNNING" } else { "🔴 Bot is STOPPED" };
        
        let keyboard = json!({
            "inline_keyboard": [[
                { "text": "▶️ Start", "callback_data": "start" },
                { "text": "⏹️ Stop", "callback_data": "stop" },
                { "text": "📊 Status", "callback_data": "status" }
            ], [
                { "text": "💰 Positions & PnL", "callback_data": "positions" }
            ]]
        });

        let params = json!({
            "chat_id": self.chat_id,
            "text": format!("🤖 *Bot Control Panel*\n\nCurrent Status: {}", status_text),
            "parse_mode": "Markdown",
            "reply_markup": keyboard
        });

        self.client.post(&url).json(&params).send().await?;
        Ok(())
    }

    pub async fn send_control_keyboard_to(&self, chat_id: i64, is_running: bool) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        
        let status_text = if is_running { "🟢 Bot is RUNNING" } else { "🔴 Bot is STOPPED" };
        
        let keyboard = json!({
            "inline_keyboard": [[
                { "text": "▶️ Start", "callback_data": "start" },
                { "text": "⏹️ Stop", "callback_data": "stop" },
                { "text": "📊 Status", "callback_data": "status" }
            ], [
                { "text": "💰 Positions & PnL", "callback_data": "positions" }
            ]]
        });

        let params = json!({
            "chat_id": chat_id,
            "text": format!("🤖 *Bot Control Panel*\n\nCurrent Status: {}", status_text),
            "parse_mode": "Markdown",
            "reply_markup": keyboard
        });

        self.client.post(&url).json(&params).send().await?;
        Ok(())
    }

    pub async fn send_message_with_menu_btn(&self, chat_id: i64, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        
        let keyboard = json!({
            "inline_keyboard": [[
                { "text": "🔙 Menu", "callback_data": "menu" }
            ]]
        });

        let params = json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown",
            "reply_markup": keyboard
        });

        self.client.post(&url).json(&params).send().await?;
        Ok(())
    }

    pub async fn send_default_message_with_menu_btn(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        
        let keyboard = json!({
            "inline_keyboard": [[
                { "text": "🔙 Menu", "callback_data": "menu" }
            ]]
        });

        let params = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown",
            "reply_markup": keyboard
        });

        self.client.post(&url).json(&params).send().await?;
        Ok(())
    }

    pub async fn run_listener(self, is_running: Arc<AtomicBool>, position_manager: Arc<Mutex<PositionManager>>) {
        let mut offset = 0;
        println!("📱 Telegram Listener Started...");

        loop {
            let url = format!("https://api.telegram.org/bot{}/getUpdates", self.token);
            let params = json!({
                "offset": offset,
                "timeout": 30
            });

            match self.client.post(&url).json(&params).send().await {
                Ok(resp) => {
                    if let Ok(updates) = resp.json::<serde_json::Value>().await {
                        if let Some(result) = updates["result"].as_array() {
                            for update in result {
                                offset = update["update_id"].as_i64().unwrap_or(0) + 1;

                                // Handle Callback Queries (Buttons)
                                if let Some(callback) = update.get("callback_query") {
                                    let data = callback["data"].as_str().unwrap_or("");
                                    let callback_id = callback["id"].as_str().unwrap_or("");
                                    let message = callback.get("message");
                                    let chat_id = message.and_then(|m| m.get("chat")).and_then(|c| c.get("id")).and_then(|i| i.as_i64());
                                    
                                    let mut reply_text = "";

                                    match data {
                                        "start" => {
                                            is_running.store(true, Ordering::SeqCst);
                                            reply_text = "✅ Bot STARTED";
                                            if let Some(cid) = chat_id {
                                                let _ = self.send_message_with_menu_btn(cid, "🟢 *Bot Started* - Trading is now active.").await;
                                            } else {
                                                let _ = self.send_default_message_with_menu_btn("🟢 *Bot Started* - Trading is now active.").await;
                                            }
                                        },
                                        "stop" => {
                                            is_running.store(false, Ordering::SeqCst);
                                            reply_text = "🛑 Bot STOPPED";
                                            if let Some(cid) = chat_id {
                                                let _ = self.send_message_with_menu_btn(cid, "🔴 *Bot Stopped* - Trading is paused.").await;
                                            } else {
                                                let _ = self.send_default_message_with_menu_btn("🔴 *Bot Stopped* - Trading is paused.").await;
                                            }
                                        },
                                        "status" => {
                                            let running = is_running.load(Ordering::SeqCst);
                                            let status = if running { "RUNNING 🟢" } else { "STOPPED 🔴" };
                                            if let Some(cid) = chat_id {
                                                let _ = self.send_message_with_menu_btn(cid, &format!("📊 Status: {}", status)).await;
                                            } else {
                                                let _ = self.send_default_message_with_menu_btn(&format!("📊 Status: {}", status)).await;
                                            }
                                            reply_text = "Status Sent";
                                        },
                                        "positions" => {
                                            let pm = position_manager.lock().await;
                                            let mut msg = format!("💰 *Bankroll Info*\n\nTotal Balance: ${:.2}\nAvailable: ${:.2}\n\n", 
                                                pm.bankroll.total_balance, pm.bankroll.available_balance);
                                            
                                            if let Some(pos) = &pm.position {
                                                let state_str = match pos.state {
                                                    crate::position_manager::PositionState::Long => "🟢 LONG",
                                                    crate::position_manager::PositionState::Short => "📉 SHORT",
                                                    _ => "⚪ NONE",
                                                };
                                                
                                                msg.push_str(&format!("*Current Position:*\nState: {}\nEntry: ${:.2}\nSize: {:.4} SOL\nUnrealized PnL: ${:+.2} ({:+.2}%)",
                                                    state_str, pos.entry_price, pos.position_size, pos.unrealized_pnl, pos.unrealized_pnl_pct));
                                            } else {
                                                msg.push_str("*Current Position:* None");
                                            }
                                            
                                            if let Some(cid) = chat_id {
                                                let _ = self.send_message_with_menu_btn(cid, &msg).await;
                                            } else {
                                                let _ = self.send_default_message_with_menu_btn(&msg).await;
                                            }
                                            reply_text = "Positions Sent";
                                        },
                                        "menu" => {
                                            let running = is_running.load(Ordering::SeqCst);
                                            if let Some(cid) = chat_id {
                                                let _ = self.send_control_keyboard_to(cid, running).await;
                                            } else {
                                                let _ = self.send_control_keyboard(running).await;
                                            }
                                            reply_text = "Menu Opened";
                                        },
                                        _ => {}
                                    }

                                    // Answer callback to stop loading animation
                                    let answer_url = format!("https://api.telegram.org/bot{}/answerCallbackQuery", self.token);
                                    let _ = self.client.post(&answer_url).json(&json!({
                                        "callback_query_id": callback_id,
                                        "text": reply_text
                                    })).send().await;
                                }
                                // Handle Text Commands
                                else if let Some(message) = update.get("message") {
                                    if let Some(text) = message["text"].as_str() {
                                        println!("📩 Received message: {}", text);
                                        if text == "/start" || text == "/menu" {
                                            let running = is_running.load(Ordering::SeqCst);
                                            
                                            // Try to reply to the sender
                                            if let Some(chat_id) = message["chat"]["id"].as_i64() {
                                                println!("   From Chat ID: {}", chat_id);
                                                let _ = self.send_control_keyboard_to(chat_id, running).await;
                                            } else {
                                                // Fallback to default chat_id
                                                let _ = self.send_control_keyboard(running).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("⚠️ Telegram Poll Error: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn send_message_to(&self, chat_id: i64, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        let params = json!({
            "chat_id": chat_id,
            "text": message,
            "parse_mode": "Markdown"
        });

        self.client.post(&url).json(&params).send().await?;
        Ok(())
    }
}
