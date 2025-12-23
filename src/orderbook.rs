use crate::interfaces::{OrderBook, Price, Quantity, Side, Update};

pub struct OrderBookImpl {
    bids: Vec<Quantity>,
    asks: Vec<Quantity>,
    best_bid_idx: Option<usize>,
    best_ask_idx: Option<usize>,
    min_price: i64,
    max_price: i64,
}

impl OrderBookImpl {
    /// Create a new orderbook with custom price range
    /// 
    /// # Arguments
    /// * `min_price` - Minimum price (in i64 with 4 decimals, e.g., 1000000 for $100)
    /// * `max_price` - Maximum price (in i64 with 4 decimals)
    pub fn with_range(min_price: i64, max_price: i64) -> Self {
        let levels = (max_price - min_price) as usize;
        Self {
            bids: vec![0; levels],
            asks: vec![0; levels],
            best_bid_idx: None,
            best_ask_idx: None,
            min_price,
            max_price,
        }
    }

    #[inline(always)]
    fn price_to_idx(&self, price: Price) -> Option<usize> {
        if price >= self.min_price && price < self.max_price {
            Some((price - self.min_price) as usize)
        } else {
            None
        }
    }
    
    #[inline(always)]
    fn idx_to_price(&self, idx: usize) -> Price {
        self.min_price + idx as i64
    }
    
    #[inline(always)]
    fn price_levels(&self) -> usize {
        self.bids.len()
    }
}

impl OrderBook for OrderBookImpl {
    /// Create orderbook with default range (0 to $20)
    /// For most use cases, prefer using `with_range()` for proper price support
    fn new() -> Self {
        Self::with_range(0, 200_000)
    }

    #[inline(always)]
    fn apply_update(&mut self, update: Update) {
        match update {
            Update::Set { price, quantity, side } => {
                if let Some(idx) = self.price_to_idx(price) {
                    let levels = self.price_levels();
                    unsafe {
                        match side {
                            Side::Bid => {
                                *self.bids.get_unchecked_mut(idx) = quantity;
                                
                                if quantity > 0 {
                                    self.best_bid_idx = Some(match self.best_bid_idx {
                                        Some(best) if idx > best => idx,
                                        Some(best) => best,
                                        None => idx,
                                    });
                                } else if Some(idx) == self.best_bid_idx {
                                    self.best_bid_idx = (0..idx)
                                        .rev()
                                        .find(|&i| *self.bids.get_unchecked(i) > 0);
                                }
                            }
                            Side::Ask => {
                                *self.asks.get_unchecked_mut(idx) = quantity;
                                
                                if quantity > 0 {
                                    self.best_ask_idx = Some(match self.best_ask_idx {
                                        Some(best) if idx < best => idx,
                                        Some(best) => best,
                                        None => idx,
                                    });
                                } else if Some(idx) == self.best_ask_idx {
                                    self.best_ask_idx = (idx + 1..levels)
                                        .find(|&i| *self.asks.get_unchecked(i) > 0);
                                }
                            }
                        }
                    }
                }
            }
            Update::Remove { price, side } => {
                if let Some(idx) = self.price_to_idx(price) {
                    unsafe {
                        match side {
                            Side::Bid => {
                                *self.bids.get_unchecked_mut(idx) = 0;
                                if Some(idx) == self.best_bid_idx {
                                    self.best_bid_idx = (0..idx)
                                        .rev()
                                        .find(|&i| *self.bids.get_unchecked(i) > 0);
                                }
                            }
                            Side::Ask => {
                                *self.asks.get_unchecked_mut(idx) = 0;
                                if Some(idx) == self.best_ask_idx {
                                    let levels = self.price_levels();
                                    self.best_ask_idx = (idx + 1..levels)
                                        .find(|&i| *self.asks.get_unchecked(i) > 0);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn get_spread(&self) -> Option<Price> {
        match (self.best_ask_idx, self.best_bid_idx) {
            (Some(ask_idx), Some(bid_idx)) => {
                Some(self.idx_to_price(ask_idx) - self.idx_to_price(bid_idx))
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn get_best_bid(&self) -> Option<Price> {
        self.best_bid_idx.map(|idx| self.idx_to_price(idx))
    }

    #[inline(always)]
    fn get_best_ask(&self) -> Option<Price> {
        self.best_ask_idx.map(|idx| self.idx_to_price(idx))
    }

    #[inline(always)]
    fn get_quantity_at(&self, price: Price, side: Side) -> Option<Quantity> {
        self.price_to_idx(price).and_then(|idx| {
            unsafe {
                let qty = match side {
                    Side::Bid => *self.bids.get_unchecked(idx),
                    Side::Ask => *self.asks.get_unchecked(idx),
                };
                if qty > 0 { Some(qty) } else { None }
            }
        })
    }

    fn get_top_levels(&self, side: Side, n: usize) -> Vec<(Price, Quantity)> {
        let mut result = Vec::with_capacity(n);
        let levels = self.price_levels();
        
        match side {
            Side::Bid => {
                if let Some(start) = self.best_bid_idx {
                    for idx in (0..=start).rev() {
                        if self.bids[idx] > 0 {
                            result.push((self.idx_to_price(idx), self.bids[idx]));
                            if result.len() >= n {
                                break;
                            }
                        }
                    }
                }
            }
            Side::Ask => {
                if let Some(start) = self.best_ask_idx {
                    for idx in start..levels {
                        if self.asks[idx] > 0 {
                            result.push((self.idx_to_price(idx), self.asks[idx]));
                            if result.len() >= n {
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        result
    }

    fn get_total_quantity(&self, side: Side) -> Quantity {
        match side {
            Side::Bid => self.bids.iter().sum(),
            Side::Ask => self.asks.iter().sum(),
        }
    }
}

impl Clone for OrderBookImpl {
    fn clone(&self) -> Self {
        Self {
            bids: self.bids.clone(),
            asks: self.asks.clone(),
            best_bid_idx: self.best_bid_idx,
            best_ask_idx: self.best_ask_idx,
            min_price: self.min_price,
            max_price: self.max_price,
        }
    }
}
