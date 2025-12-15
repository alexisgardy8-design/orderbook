// 🧪 Test Limit Order with Stop Loss on Hyperliquid

use crate::hyperliquid_trade::HyperliquidTrader;
use serde_json::json;

/// Fetch current SOL price
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
    { Err("WebSocket feature required".into()) }
}

pub async fn run_test_sl_order() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 HYPERLIQUID LIMIT + STOP LOSS TEST                       ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    let trader = HyperliquidTrader::new()?;

    println!("\n💹 Fetching current SOL price...");
    let current_price = match get_sol_price().await {
        Ok(p) => p,
        Err(e) => {
            println!("⚠️  Failed to fetch price: {}, using default 135.0", e);
            135.0
        }
    };
    println!("   Current Price: ${:.2}", current_price);

    let coin = "SOL";
    let sz = 0.1; // Small size
    // Limit Buy Price: 10% below market (safe)
    let limit_px = (current_price * 0.9 * 100.0).round() / 100.0;
    // Stop Loss Price: 5% below Limit Price
    let sl_px = (limit_px * 0.95 * 100.0).round() / 100.0;

    println!("\n💰 Test Scenario:");
    println!("   1. Place BUY Limit Order at ${:.2}", limit_px);
    println!("   2. Place SELL Stop Loss at ${:.2} (5% loss)", sl_px);
    
    // Step 1: Place Limit Order
    println!("\n👉 Step 1: Placing Limit Order...");
    let limit_oid = trader.place_limit_order(coin, true, limit_px, sz).await?;
    
    if limit_oid == 0 {
        println!("❌ Limit order failed or filled immediately. Aborting SL test.");
        return Ok(());
    }

    // Step 2: Place Stop Loss Order
    println!("\n👉 Step 2: Placing Stop Loss Order...");
    // Note: This might fail if we have no position and it's reduce-only.
    // But we are testing the *logic* of sending the order.
    let sl_result = trader.place_stop_loss_order(coin, false, sl_px, sz).await;
    
    let sl_oid = match sl_result {
        Ok(oid) => {
            println!("✅ Stop Loss Order Placed! OID: {}", oid);
            oid
        },
        Err(e) => {
            println!("⚠️  Stop Loss Order Failed (Expected if no position): {}", e);
            0
        }
    };

    // Step 3: Cleanup (Cancel orders)
    println!("\n🧹 Cleanup: Cancelling orders...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    if limit_oid != 0 {
        match trader.cancel_order(coin, limit_oid).await {
            Ok(_) => println!("✅ Limit Order Cancelled"),
            Err(e) => println!("❌ Failed to cancel Limit Order: {}", e),
        }
    }

    if sl_oid != 0 {
        match trader.cancel_order(coin, sl_oid).await {
            Ok(_) => println!("✅ Stop Loss Order Cancelled"),
            Err(e) => println!("❌ Failed to cancel SL Order: {}", e),
        }
    }

    println!("\n✅ Test Complete");
    Ok(())
}
