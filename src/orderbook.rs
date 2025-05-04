use crate::models::{Order, OrderBook, Side, Trade, OrderError, OrderType};
use bigdecimal::BigDecimal;
use std::cmp::Ordering;
use uuid::Uuid;

impl OrderBook{
    pub fn new (symbol: String) -> Self {
        OrderBook {
            symbol,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }
    
    pub fn add_order(&mut self, order: Order) -> Result<Vec<Trade>, OrderError> {
        match order.side {
            Side::Buy => self.match_buy_order(order),
            Side::Sell => self.match_sell_order(order),
        }
    }
        
    fn match_buy_order(&mut self, mut order: Order) -> Result<Vec<Trade>, OrderError> {
        let mut trades = Vec::new();
        let mut remaining_quantity = order.quantity.clone();

        // if even the lowest ask is higher than the price, we cant match ofcc
        for ask in self.asks.iter_mut() {
            if order.order_type == OrderType::Limit && ask.price > order.price {
                break;
            }

            let fill_quantity = if ask.quantity.clone() - ask.filled_quantity.clone() < remaining_quantity {
                ask.quantity.clone() - ask.filled_quantity.clone()
            } else {
                remaining_quantity.clone()
            };

            // making the tradee and adding filled order:
            let trade = Trade {
                id: Uuid::new_v4(),
                symbol: self.symbol.clone(),
                buyer_order_id: order.id,
                seller_order_id: ask.id,
                price: ask.price.clone(),
                quantity: fill_quantity.clone(),
                executed_at: chrono::Utc::now(),
            };

            trades.push(trade);
            ask.filled_quantity += &fill_quantity;
            order.filled_quantity += &fill_quantity;
            remaining_quantity -= fill_quantity;

            if remaining_quantity <= BigDecimal::from(0) {
                break;
            }
        }

        // clearing fully filled asks
        self.asks.retain(|o| o.filled_quantity < o.quantity);

        // quanitiy for the buy order is still greater than 0, then add the order to the book:
        if remaining_quantity > BigDecimal::from(0) 
            && order.order_type == OrderType::Limit {
            order.quantity = remaining_quantity;
            self.bids.push(order);
            self.bids.sort_by(|a, b| b.price.cmp(&a.price));
        }

        Ok(trades)

    }

    fn match_sell_order(&mut self, mut order: Order) -> Result<Vec<Trade>, OrderError> {
        let mut trades = Vec::new();
        let mut remaining_quantity = order.quantity.clone();

        for bid in self.bids.iter_mut() {

            // if even the highest big price is lower than the ask then bruhh you ngmi brugh:
            if order.order_type == OrderType::Limit && bid.price < order.price {
                break;
            }

            let fill_quantity: BigDecimal = if bid.quantity.clone() - bid.filled_quantity.clone() < remaining_quantity.clone() {
                bid.quantity.clone() - bid.filled_quantity.clone()
            } else {
                remaining_quantity.clone()
            };

            let trade = Trade {
                id: Uuid::new_v4(),
                symbol: self.symbol.clone(),
                buyer_order_id: bid.id,
                seller_order_id: order.id,
                price: bid.price.clone(),
                quantity: fill_quantity.clone(),
                executed_at: chrono::Utc::now(),
            };

            trades.push(trade);

            bid.filled_quantity += &fill_quantity;
            order.filled_quantity += &fill_quantity;
            remaining_quantity -= fill_quantity;

            if remaining_quantity <= BigDecimal::from(0) {
                break;
            }            
            
        }
        self.bids.retain(|o| o.filled_quantity < o.quantity);

        // Add remaining order to book if limit order with remaining quantity
        if remaining_quantity > BigDecimal::from(0) 
            && order.order_type == OrderType::Limit {
            order.quantity = remaining_quantity;
            self.asks.push(order);
            self.asks.sort_by(|a, b| a.price.cmp(&b.price)); // Ascending for asks
        }

        Ok(trades)
    }

    pub fn cancel_order(&mut self, order_id: Uuid, side: Side) -> Result<(), OrderError> {
        match side {
            Side::Buy => {
                if let Some(pos) = self.bids.iter().position(|o| o.id == order_id) {
                    self.bids.remove(pos);
                    Ok(())
                } else {
                    Err(OrderError::OrderNotFound)
                }
            }
            Side::Sell => {
                if let Some(pos) = self.asks.iter().position(|o| o.id == order_id) {
                    self.asks.remove(pos);
                    Ok(())
                } else {
                    Err(OrderError::OrderNotFound)
                }
            }
        }
    }

    /**
     * top bids and asks
     */
    pub fn get_depth(&self, depth: usize) -> (Vec<(BigDecimal, BigDecimal)>, Vec<(BigDecimal, BigDecimal)>) {
        let bids = self.bids.iter()
            .take(depth)
            .map(|o| (o.price.clone(), o.quantity.clone() - o.filled_quantity.clone()))
            .collect();

        let asks = self.asks.iter()
            .take(depth)
            .map(|o| (o.price.clone(), o.quantity.clone() - o.filled_quantity.clone()))
            .collect();

        (bids, asks)
    }

}