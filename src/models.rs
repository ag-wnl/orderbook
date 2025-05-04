use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use std::str::FromStr;
use thiserror::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
    Stop,
    StopLimit,
}

/**
 * GTC (Good Till Cancel) - order stays active until filled or canceled
 * IOC (Immediate Or Cancel) - fills immediately whatever it can, cancels the rest
 * FOK (Fill Or Kill) - must fill completely or not at all
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,
    IOC, 
    FOK,
}

/*
* trading order metadata
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub user_id: Uuid,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub filled_quantity: BigDecimal,
    pub leverage: Option<BigDecimal>,
    pub time_in_force: TimeInForce,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/**
 * Record of a transaction
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: Uuid,
    pub symbol: String,
    pub buyer_order_id: Uuid,
    pub seller_order_id: Uuid,
    pub price: BigDecimal,
    pub quantity: BigDecimal,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

/**
 * open market position for a user
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub user_id: Uuid,
    pub symbol: String,
    pub side: Side,
    pub quantity: BigDecimal,
    pub entry_price: BigDecimal,
    pub leverage: BigDecimal,
    pub liquidation_price: BigDecimal,
    pub margin: BigDecimal,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/**
 * manage user balances and positions
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub user_id: Uuid,
    pub balances: HashMap<String, BigDecimal>, // asset -> balance
    pub positions: HashMap<String, Position>,  // token -> position
}

/**
 * order book for a token
 * bids: sorted highest to lowest price
 * asks: sorted lowest to highest price
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: Vec<Order>, 
    pub asks: Vec<Order>, 
}

/**
 * funding rate for a token - 
 * for a perp, funding rate = interest rate paid or received by the longs or shorts
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub rate: BigDecimal,
    pub next_funding_time: chrono::DateTime<chrono::Utc>,
}

/**
 * errors when can occur when processing orders
 */
#[derive(Debug, Error)]
pub enum OrderError {
    #[error("Insufficient balance")]
    InsufficientBalance,
    #[error("Invalid order parameters")]
    InvalidOrder,
    #[error("Order not found")]
    OrderNotFound,
    #[error("Position would be liquidated")]
    WouldLiquidate,
    #[error("Funding payment failed")]
    FundingError,
}

// formatterr
impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

// parserrr
impl FromStr for Side {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "BUY" => Ok(Side::Buy),
            "SELL" => Ok(Side::Sell),
            _ => Err(format!("'{}' is not a valid side", s)),
        }
    }
}




