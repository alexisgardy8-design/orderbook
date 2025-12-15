// 🧪 Test Market Cycle: Open Market -> Place SL -> Close Market
// Uses 2x Leverage to handle small bankroll ($8)

use crate::hyperliquid_trade::HyperliquidTrader;
use crate::adaptive_strategy::{AdaptiveStrategy, AdaptiveConfig};
use crate::hyperliquid_historical::HyperliquidHistoricalData;
#[cfg(feature = "websocket")]
use crate::telegram::TelegramBot;
use serde_json::json;

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

pub async fn run_test_market_cycle() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║  🧪 HYPERLIQUID MARKET CYCLE TEST (2x LEVERAGE)              ║");
    println!("╚════════════════════════════════════════════════════════════════╝");

    let trader = HyperliquidTrader::new()?;
    let coin = "SOL";

    // 0. Initialize Strategy and Warmup
    println!("\n🧠 Initializing Strategy and Warming up...");
    let config = AdaptiveConfig::default();
    let mut strategy = AdaptiveStrategy::new(config);
    
    let historical = HyperliquidHistoricalData::new(coin.to_string(), "1h".to_string());
    // Use spawn_blocking because fetch_recent_candles is blocking (ureq)
    let candles = tokio::task::spawn_blocking(move || {
        historical.fetch_recent_candles().map_err(|e| e.to_string())
    }).await??;

    println!("   Feeding {} candles to strategy...", candles.len());
    for candle in candles {
        let (o, h, l, c, _v) = candle.to_ohlc().unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0));
        strategy.update(h, l, c);
    }
    println!("   Strategy warmed up. Current ADX: {:.2}", strategy.get_adx_value());

    // 1. Get Price (and verify connectivity)
    println!("\n💹 Fetching current SOL price...");
    let current_price = match get_sol_price().await {
        Ok(p) => p,
        Err(e) => {
            println!("⚠️  Failed to fetch price: {}", e);
            return Err(e);
        }
    };
    println!("   Current Price: ${:.2}", current_price);

    // 2. Update Leverage to 2x (Isolated)
    println!("\n⚙️  Step 1: Setting Leverage to 2x (Isolated)...");
    trader.update_leverage(coin, 2, false).await?;

    // 3. Calculate Size
    // Target $15 position size (requires ~$7.5 margin at 2x)
    // Min order value is $10, so $15 is safe.
    let target_value_usd = 15.0;
    let sz = target_value_usd / current_price;
    // Round to 2 decimals (SOL precision)
    let sz = (sz * 100.0).round() / 100.0;
    
    println!("\n💰 Trade Details:");
    println!("   Target Value: ${:.2}", target_value_usd);
    println!("   Size:         {:.2} SOL", sz);
    println!("   Leverage:     2x");

    if sz < 0.01 {
        return Err("Calculated size too small (< 0.01 SOL)".into());
    }

    // 4. Open Market Order (Buy)
    println!("\n👉 Step 2: Opening Position (Market BUY)...");
    let open_oid = trader.place_market_order(coin, true, sz, current_price, 0.05).await?;
    
    if open_oid == 0 {
        return Err("Failed to place open order".into());
    }
    println!("✅ Position Opened! OID: {}", open_oid);

    // 5. Place Stop Loss (Simulating integration)
    println!("\n👉 Step 3: Placing Stop Loss (-5%)...");
    let sl_price = (current_price * 0.95 * 100.0).round() / 100.0;
    let sl_oid = match trader.place_stop_loss_order(coin, false, sl_price, sz).await {
        Ok(oid) => {
            println!("✅ Stop Loss Placed! OID: {}", oid);
            oid
        },
        Err(e) => {
            println!("⚠️  Failed to place SL: {}", e);
            0
        }
    };

    // 6. Wait a bit
    println!("\n⏳ Waiting 5 seconds before closing...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // 7. Close Position (Market Sell)
    println!("\n👉 Step 4: Closing Position (Market SELL)...");
    // Fetch fresh price for better accuracy
    let close_price = get_sol_price().await.unwrap_or(current_price);
    let close_oid = trader.place_market_order(coin, false, sz, close_price, 0.05).await?;
    
    if close_oid != 0 {
        println!("✅ Position Closed! OID: {}", close_oid);
    } else {
        println!("⚠️  Failed to place close order (might have filled immediately or failed)");
    }

    // 8. Cancel SL if it exists
    if sl_oid != 0 {
        println!("\n🧹 Cleanup: Cancelling SL...");
        match trader.cancel_order(coin, sl_oid).await {
            Ok(_) => println!("✅ SL Cancelled"),
            Err(e) => println!("⚠️  Failed to cancel SL (might be triggered or cancelled by reduce-only): {}", e),
        }
    }

    // 9. Print Strategy Indicators
    println!("\n📊 Strategy Indicators at Close:");
    println!("   ADX:           {:.2}", strategy.get_adx_value());
    println!("   RSI:           {:.2}", strategy.get_rsi_value());
    
    let pnl = (close_price - current_price) * sz;
    let pnl_color = if pnl >= 0.0 { "🟢" } else { "🔴" };
    println!("   Realized PnL:  {} ${:.4} (Approx)", pnl_color, pnl);

    let regime = strategy.get_current_regime();
    println!("   Market Regime: {:?}", regime);
    
    if let crate::adaptive_strategy::MarketRegime::Ranging = regime {
        if let Some((lower, middle, upper)) = strategy.get_bollinger_bands() {
            println!("   Bollinger Bands (H1):");
            println!("     Upper: ${:.2}", upper);
            println!("     Middle: ${:.2}", middle);
            println!("     Lower: ${:.2}", lower);
            
            if close_price > upper {
                println!("     Status: 🔴 PRICE ABOVE UPPER BAND (Overbought)");
            } else if close_price < lower {
                println!("     Status: 🟢 PRICE BELOW LOWER BAND (Oversold)");
            } else {
                println!("     Status: ⚪ PRICE INSIDE BANDS");
            }
        }
    }

    println!("\n✅ Market Cycle Test Complete");

    #[cfg(feature = "websocket")]
    {
        if let Some(bot) = TelegramBot::new() {
            println!("\n📱 Sending Telegram Report...");
            let msg = format!(
                "🧪 *Market Cycle Test Complete*\n\n\
                *Coin:* {}\n\
                *Action:* Buy -> Sell\n\
                *Entry:* ${:.2}\n\
                *Exit:* ${:.2}\n\
                *PnL:* {} ${:.4}\n\
                *RSI:* {:.2}\n\
                *ADX:* {:.2}",
                coin, current_price, close_price, pnl_color, pnl, strategy.get_rsi_value(), strategy.get_adx_value()
            );
            if let Err(e) = bot.send_message(&msg).await {
                println!("⚠️ Failed to send Telegram message: {}", e);
            } else {
                println!("✅ Telegram Report Sent!");
            }
        }
    }

    Ok(())
}
