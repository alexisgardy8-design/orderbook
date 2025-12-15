// ⚡ Order Execution - Ultra-fast order execution in DRY RUN mode
// Simulated orders for testing, real API calls ready for production

use serde::{Deserialize, Serialize};

/// Order request structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HyperliquidOrder {
    pub action: String,
    pub orders: Vec<OrderRequest>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderRequest {
    pub coin: String,
    #[serde(rename = "isBuy")]
    pub is_buy: bool,
    pub sz: String,
    pub limit_px: String,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    #[serde(rename = "orderType")]
    pub order_type: String,
}

/// Order response from API
#[derive(Debug, Deserialize)]
pub struct OrderResponse {
    pub status: String,
    pub response: OrderResponseData,
}

#[derive(Debug, Deserialize)]
pub struct OrderResponseData {
    #[serde(rename = "orderIds")]
    pub order_ids: Vec<u64>,
}

/// Simulated order for DRY RUN mode
#[derive(Debug, Clone)]
pub struct SimulatedOrder {
    pub order_id: u64,
    pub side: String,
    pub size: f64,
    pub execution_price: f64,
    pub status: String,
    pub timestamp: u64,
}

/// Order executor for live trading
pub struct OrderExecutor {
    next_order_id: u64,
    simulator: OrderSimulator,
}

impl OrderExecutor {
    pub fn new() -> Self {
        Self {
            next_order_id: 1,
            simulator: OrderSimulator::new(),
        }
    }

    /// Execute a market order (immediate execution)
    pub fn market_order(
        &mut self,
        is_buy: bool,
        size: f64,
        current_price: f64,
    ) -> Result<u64, String> {
        // In DRY RUN, we always simulate
        // In production, this would call Hyperliquid API
        
        let simulated = self.simulator.market_order(is_buy, size, current_price);
        
        println!("   ✅ Order SIMULATED: {} {} SOL @ ${:.2}", 
            if is_buy { "BUY" } else { "SELL" },
            size,
            simulated.execution_price
        );
        Ok(simulated.order_id)
    }

    /// Execute a limit order
    pub fn limit_order(
        &mut self,
        is_buy: bool,
        size: f64,
        limit_price: f64,
        current_price: f64,
    ) -> Result<u64, String> {
        let simulated = self.simulator.limit_order(is_buy, size, limit_price, current_price);
        
        if let Some(order) = simulated {
            println!("   ✅ Order SIMULATED (Limit): {} {} SOL @ ${:.2}", 
                if is_buy { "BUY" } else { "SELL" },
                size,
                order.execution_price
            );
            Ok(order.order_id)
        } else {
            Err("Limit order not filled".to_string())
        }
    }

    /// Close a position
    pub fn close_position(
        &mut self,
        position_size: f64,
        is_long: bool,
        current_price: f64,
    ) -> Result<u64, String> {
        let is_buy = !is_long; // Reverse direction
        
        let simulated = self.simulator.market_order(is_buy, position_size, current_price);
        
        println!("   ✅ Position CLOSED (SIMULATED): {} SOL @ ${:.2}", 
            position_size,
            simulated.execution_price
        );
        Ok(simulated.order_id)
    }
}

/// Simulator for DRY RUN mode
pub struct OrderSimulator {
    next_order_id: u64,
}

impl OrderSimulator {
    pub fn new() -> Self {
        Self {
            next_order_id: 1,
        }
    }

    /// Simulate a market order
    pub fn market_order(
        &mut self,
        is_buy: bool,
        size: f64,
        current_price: f64,
    ) -> SimulatedOrder {
        let order_id = self.next_order_id;
        self.next_order_id += 1;

        // Realistic slippage for SOL-PERP
        let execution_price = if is_buy {
            current_price * 1.0001  // Slight slippage for buy
        } else {
            current_price * 0.9999  // Slight slippage for sell
        };

        let side = if is_buy { "BUY" } else { "SELL" };
        
        SimulatedOrder {
            order_id,
            side: side.to_string(),
            size,
            execution_price,
            status: "FILLED".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        }
    }

    /// Simulate a limit order
    pub fn limit_order(
        &mut self,
        is_buy: bool,
        size: f64,
        limit_price: f64,
        current_price: f64,
    ) -> Option<SimulatedOrder> {
        // Order only executes if price is favorable
        let can_execute = if is_buy {
            current_price <= limit_price
        } else {
            current_price >= limit_price
        };

        if !can_execute {
            return None; // Order pending
        }

        let order_id = self.next_order_id;
        self.next_order_id += 1;

        let side = if is_buy { "BUY" } else { "SELL" };

        Some(SimulatedOrder {
            order_id,
            side: side.to_string(),
            size,
            execution_price: limit_price,
            status: "FILLED".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        })
    }
}
