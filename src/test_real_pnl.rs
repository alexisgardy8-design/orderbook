// 🧪 Test Real PnL Calculation
// Opens a small position, closes it, and calculates exact PnL from fills

use crate::hyperliquid_trade::HyperliquidTrader;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

/// Fetch current SOL price
async fn get_sol_price() -> Result<f64, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.hyperliquid.xyz/info")
        .json(&json!({"type": "allMids"}))
        .send()
        .await?;
    
    let data: serde_json::Value = response.json().await?;
    let price = data["SOL"].as_str()
        .ok_or("SOL price not found")?
        .parse::<f64>()?;
    Ok(price)
}

pub async fn run_test_pnl() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 HYPERLIQUID REAL PNL TEST                                ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    let trader = HyperliquidTrader::new()?;
    let coin = "SOL";
    
    // 1. Get Price
    let price = get_sol_price().await?;
    println!("   Current Price: ${:.2}", price);

    // 2. Calculate Size (Min $12 to be safe)
    let size = (12.0 / price).max(0.01);
    let size = (size * 100.0).ceil() / 100.0; // Round up to 2 decimals
    println!("   Test Size:     {:.2} SOL (~${:.2})", size, size * price);

    let start_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64;

    // 3. Open Position (Market Buy)
    println!("\n🚀 Opening Position (Market Buy)...");
    let oid_open = trader.place_market_order(coin, true, size, price, 0.05).await?;
    println!("   ✅ Order Placed: {}", oid_open);

    println!("⏳ Waiting 5 seconds...");
    sleep(Duration::from_secs(5)).await;

    // 4. Close Position (Market Sell)
    println!("\n🚀 Closing Position (Market Sell)...");
    let current_price = get_sol_price().await?;
    let oid_close = trader.place_market_order(coin, false, size, current_price, 0.05).await?;
    println!("   ✅ Order Placed: {}", oid_close);

    println!("⏳ Waiting 3 seconds for fills...");
    sleep(Duration::from_secs(3)).await;

    // 5. Fetch Data
    println!("\n📊 Fetching Fills & Funding...");
    let fills = trader.get_user_fills().await?;
    let fundings = trader.get_user_funding(start_time).await?;

    // 6. Calculate PnL
    let mut entry_price = 0.0;
    let mut exit_price = 0.0;
    let mut entry_fee = 0.0;
    let mut exit_fee = 0.0;
    let mut realized_pnl = 0.0;
    let mut funding_paid = 0.0;

    // Filter fills for this test (last 1 minute)
    let recent_fills: Vec<_> = fills.into_iter()
        .filter(|f| f.coin == coin && f.time > start_time)
        .collect();

    for fill in &recent_fills {
        let px = fill.px.parse::<f64>()?;
        let fee = fill.fee.parse::<f64>()?;
        let sz = fill.sz.parse::<f64>()?;

        if fill.side == "B" {
            entry_price = px;
            entry_fee += fee;
            println!("   📝 Buy Fill:  {:.2} SOL @ ${:.2} (Fee: ${:.4})", sz, px, fee);
        } else {
            exit_price = px;
            exit_fee += fee;
            if let Some(pnl) = &fill.closed_pnl {
                realized_pnl += pnl.parse::<f64>()?;
            }
            println!("   📝 Sell Fill: {:.2} SOL @ ${:.2} (Fee: ${:.4})", sz, px, fee);
        }
    }

    for f in fundings {
        if f.coin == coin {
            let amount = f.usdc.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
            funding_paid += amount;
            println!("   💸 Funding: ${:.6}", amount);
        }
    }

    // If realized_pnl not from API (partial fills etc), estimate it
    if realized_pnl == 0.0 && exit_price > 0.0 {
        realized_pnl = (exit_price - entry_price) * size;
    }

    let total_fees = entry_fee + exit_fee;
    let net_pnl = realized_pnl - total_fees + funding_paid;

    println!("\n💰 PNL REPORT:");
    println!("   Entry Price:   ${:.2}", entry_price);
    println!("   Exit Price:    ${:.2}", exit_price);
    println!("   Gross PnL:     ${:+.4}", realized_pnl);
    println!("   Trading Fees:  -${:.4}", total_fees);
    println!("   Funding:       ${:+.4}", funding_paid);
    println!("   ━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("   NET PNL:       ${:+.4}", net_pnl);

    Ok(())
}
