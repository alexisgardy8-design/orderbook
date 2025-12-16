use crate::hyperliquid_feed::{HyperliquidFeed, HyperCandle};
use crate::position_manager::PositionManager;
use crate::supabase::SupabaseClient;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::sync::Mutex;
use std::collections::VecDeque;

#[tokio::test]
async fn test_supabase_log_on_candle_close() {
    dotenv::dotenv().ok();
    // 1. Setup Mock Environment
    let is_running = Arc::new(AtomicBool::new(true));
    let supabase = SupabaseClient::new(); // Will use .env vars
    
    if supabase.is_none() {
        eprintln!("⚠️ Skipping test: Supabase credentials not found in .env");
        return;
    }

    let pm = Arc::new(Mutex::new(PositionManager::new(1000.0, supabase.clone())));
    
    // Create a feed instance (partially mocked)
    let mut feed = HyperliquidFeed::new(
        "SOL".to_string(),
        "1h".to_string(),
        false, // Not live
        is_running,
        pm.clone(),
        None // No Telegram for this test
    );

    // 2. Create a Fake Candle (Closed)
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    let candle = HyperCandle {
        t: now - 3600000, // 1 hour ago
        close_t: now,
        s: "SOL".to_string(),
        i: "1h".to_string(),
        o: 150.0,
        h: 155.0,
        l: 149.0,
        c: 152.0,
        v: 1000.0,
        n: 100,
    };

    println!("🧪 Simulating Candle Close: ${} -> ${}", candle.o, candle.c);

    // 3. Inject Candle into Buffer manually to simulate "previous" candle
    // Fill buffer to satisfy warmup (need 50 candles)
    for _ in 0..50 {
        feed.candle_buffer.push_back(candle.clone());
    }
    
    // 4. Trigger "Process Candle" with a NEW candle to force close the previous one
    let new_candle = HyperCandle {
        t: now,
        close_t: now + 3600000,
        s: "SOL".to_string(),
        i: "1h".to_string(),
        o: 152.0,
        h: 152.5,
        l: 151.5,
        c: 152.2,
        v: 100.0,
        n: 10,
    };

    // This should trigger the "CANDLE CLOSED" logic and the Supabase Log
    feed.process_candle(new_candle, 0, false).await;

    // 5. Verify Log in Supabase
    println!("⏳ Waiting for async log to propagate...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    if let Some(client) = supabase {
        match client.fetch_last_logs(10).await {
            Ok(logs) => {
                let found = logs.iter().find(|log| log.message.contains("Candle Closed") && log.message.contains("152.00"));
                
                if let Some(log) = found {
                    println!("✅ Log Found: {}", log.message);
                } else {
                    println!("❌ Recent logs in DB:");
                    for log in logs {
                        println!(" - {}", log.message);
                    }
                    panic!("❌ Expected log 'Candle Closed ... 152.00' not found in the last 10 logs!");
                }
            },
            Err(e) => panic!("❌ Failed to fetch logs: {}", e),
        }
    }
}
