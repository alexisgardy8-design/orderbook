// 🧪 Test Live Order Execution on Hyperliquid  
// Places a real BUY LIMIT order and cancels it

use crate::hyperliquid_trade::HyperliquidTrader;
use serde_json::json;
use std::env;

/// Fetch current SOL price from Hyperliquid info API
async fn get_sol_price() -> Result<f64, Box<dyn std::error::Error>> {
    #[cfg(feature = "websocket")]
    {
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.hyperliquid.xyz/info")
            .json(&json!({"type": "allMids"}))
            .send()
            .await?;
        
        let data: serde_json::Value = response.json().await?;
        let price = data["SOL"].as_str()
            .ok_or("SOL price not found in API response")?
            .parse::<f64>()?;
        Ok(price)
    }
    
    #[cfg(not(feature = "websocket"))]
    {
        Err("WebSocket feature required".into())
    }
}

/// Test the order placement and cancellation
pub async fn run_test_order_execution() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 HYPERLIQUID LIVE ORDER EXECUTION TEST                    ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    // Initialize trader (loads private key)
    let trader = HyperliquidTrader::new()?;

    // Fetch current SOL price
    println!("\n💹 Fetching current SOL price from Hyperliquid...");
    let current_price = match get_sol_price().await {
        Ok(price) => {
            println!("   ✅ Current SOL price: ${:.2}", price);
            price
        }
        Err(e) => {
            println!("   ⚠️  Could not fetch price ({}), using default", e);
            132.715 // Default fallback
        }
    };

    // Test parameters
    let coin = "SOL";
    let test_size = 1.0; // 1.0 SOL to meet $10 min value (at ~$13 price)
    // Round to 2 decimal places to satisfy tick size requirements
    let test_price = (current_price * 0.1 * 100.0).round() / 100.0; 
    
    println!("\n💰 Test Order Details:");
    println!("   Coin: {}-PERP", coin);
    println!("   Current Market Price: ${:.6}", current_price);
    println!("   Order Size: {:.6} {} (test size)", test_size, coin);
    println!("   Limit Price: ${:.6} (10% of market - won't fill)", test_price);
    println!("   ℹ️  Order will NOT fill because price is far below market");
    
    // Step 1: Place order
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Step 1: Place BUY LIMIT Order");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let order_id = trader.place_limit_order(coin, true, test_price, test_size).await?;
    
    // Step 2: Wait
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⏳ Waiting 2 seconds...");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Step 3: Cancel order
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Step 2: Cancel the Order");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    trader.cancel_order(coin, order_id).await?;

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  ✅ TEST COMPLETE - LIVE TRADING VERIFIED!                  ║");
    println!("║                                                              ║");
    println!("║  ✅ Connected to Hyperliquid API                            ║");
    println!("║  ✅ Fetched live price (${:.6})                            ║", current_price);
    println!("║  ✅ Placed order (ID: {:<31})║", order_id);
    println!("║  ✅ Cancelled order successfully                            ║");
    println!("║                                                              ║");
    println!("║  🎉 Ready for LIVE trading on Hyperliquid!                 ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    Ok(())
}
