use crate::interfaces::{OrderBook, Price, Quantity, Side, Update};

const PRICE_LEVELS: usize = 200_000;
const MIN_PRICE: i64 = 0;
const MAX_PRICE: i64 = 200_000;

pub struct OrderBookImpl {
    bids: Vec<Quantity>,
    asks: Vec<Quantity>,
    best_bid_idx: Option<usize>,
    best_ask_idx: Option<usize>,
}

impl OrderBookImpl {
    #[inline(always)]
    fn price_to_idx(price: Price) -> Option<usize> {
        if price >= MIN_PRICE && price < MAX_PRICE {
            Some((price - MIN_PRICE) as usize)
        } else {
            None
        }
    }
    
    #[inline(always)]
    fn idx_to_price(idx: usize) -> Price {
        MIN_PRICE + idx as i64
    }
}

impl OrderBook for OrderBookImpl {
    fn new() -> Self {
        Self {
            bids: vec![0; PRICE_LEVELS],
            asks: vec![0; PRICE_LEVELS],
            best_bid_idx: None,
            best_ask_idx: None,
        }
    }

    #[inline(always)]
    fn apply_update(&mut self, update: Update) {
        match update {
            Update::Set { price, quantity, side } => {
                if let Some(idx) = Self::price_to_idx(price) {
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
                                    self.best_ask_idx = (idx + 1..PRICE_LEVELS)
                                        .find(|&i| *self.asks.get_unchecked(i) > 0);
                                }
                            }
                        }
                    }
                }
            }
            Update::Remove { price, side } => {
                if let Some(idx) = Self::price_to_idx(price) {
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
                                    self.best_ask_idx = (idx + 1..PRICE_LEVELS)
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
                Some(Self::idx_to_price(ask_idx) - Self::idx_to_price(bid_idx))
            }
            _ => None,
        }
    }

    #[inline(always)]
    fn get_best_bid(&self) -> Option<Price> {
        self.best_bid_idx.map(Self::idx_to_price)
    }

    #[inline(always)]
    fn get_best_ask(&self) -> Option<Price> {
        self.best_ask_idx.map(Self::idx_to_price)
    }

    #[inline(always)]
    fn get_quantity_at(&self, price: Price, side: Side) -> Option<Quantity> {
        Self::price_to_idx(price).and_then(|idx| {
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
        
        match side {
            Side::Bid => {
                if let Some(start) = self.best_bid_idx {
                    for idx in (0..=start).rev() {
                        if self.bids[idx] > 0 {
                            result.push((Self::idx_to_price(idx), self.bids[idx]));
                            if result.len() >= n {
                                break;
                            }
                        }
                    }
                }
            }
            Side::Ask => {
                if let Some(start) = self.best_ask_idx {
                    for idx in start..PRICE_LEVELS {
                        if self.asks[idx] > 0 {
                            result.push((Self::idx_to_price(idx), self.asks[idx]));
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
        }
    }
}
