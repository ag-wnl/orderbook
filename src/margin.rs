use crate::models::{MarginType, Side, PositionType};
use bigdecimal::BigDecimal;
use std::str::FromStr;

pub struct MarginCalculator;

impl MarginCalculator {
    pub fn calculate_required_margin(
        quantity: &BigDecimal,
        price: &BigDecimal,
        leverage: &BigDecimal,
        margin_type: MarginType,
    ) -> BigDecimal {
        
        let base_margin = quantity * price / leverage;
        match margin_type {
            MarginType::Isolated => base_margin,
            MarginType::Cross => base_margin * BigDecimal::from_str("1.1").unwrap(), // 10% buffer for cross margin
        }
    }

    pub fn calculate_liquidation_price(
        entry_price: &BigDecimal,
        side: Side,
        leverage: &BigDecimal,
        margin_type: MarginType,
    ) -> BigDecimal {
        let maintenance_margin = BigDecimal::from_str("0.005").unwrap(); // 0.5%
        let buffer = match margin_type {
            MarginType::Isolated => BigDecimal::from_str("0.001").unwrap(), // 0.1% buffer
            MarginType::Cross => BigDecimal::from_str("0.002").unwrap(),    // 0.2% buffer
        };

        match side {
            Side::Buy => {
                entry_price * (BigDecimal::from_str("1").unwrap() 
                    - BigDecimal::from_str("1").unwrap() / leverage.clone() 
                    + maintenance_margin 
                    + buffer)
            },
            Side::Sell => {
                entry_price * (BigDecimal::from_str("1").unwrap() 
                    + BigDecimal::from_str("1").unwrap() / leverage.clone() 
                    - maintenance_margin 
                    - buffer)
            }
        }
    }

    pub fn is_position_liquidated(
        current_price: &BigDecimal,
        entry_price: &BigDecimal,
        side: Side,
        leverage: &BigDecimal,
        margin_type: MarginType,
    ) -> bool {
        let liquidation_price = Self::calculate_liquidation_price(
            entry_price,
            side,
            leverage,
            margin_type,
        );

        match side {
            Side::Buy => current_price <= &liquidation_price,
            Side::Sell => current_price >= &liquidation_price,
        }
    }
} 