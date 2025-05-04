use crate::models::{Account, Position, Side, Order, OrderError, PositionType, MarginType};
use crate::margin::MarginCalculator;
use bigdecimal::BigDecimal;
use uuid::Uuid;
use std::collections::HashMap;
use std::str::FromStr;

impl Account {
    pub fn new(user_id: Uuid) -> Self {
        Account {
            user_id,
            balances: HashMap::new(),
            positions: HashMap::new(),
        }
    }

    pub fn deposit(&mut self, asset: String, amount: BigDecimal) {
        *self.balances.entry(asset).or_insert(BigDecimal::from(0)) += amount;
    }

    pub fn withdraw(&mut self, asset: String) -> BigDecimal {
        self.balances.get(&asset).cloned().unwrap_or(BigDecimal::from(0))
    }

    pub fn get_balance(&self, asset: &str) -> BigDecimal {
        self.balances.get(asset).cloned().unwrap_or(BigDecimal::from(0))
    }

    pub fn update_position(
        &mut self,
        symbol: String,
        side: Side,
        quantity: &BigDecimal,
        entry_price: &BigDecimal,
        position_type: PositionType,
        leverage: &Option<BigDecimal>,
        margin_type: &Option<MarginType>,
    ) -> Result<(), OrderError> {
        let position = self.positions.entry(symbol.clone()).or_insert(Position { 
            user_id: self.user_id,
            symbol,
            side,
            position_type,
            quantity: BigDecimal::from(0),
            entry_price: BigDecimal::from(0),
            leverage: leverage.clone(),
            liquidation_price: None,
            margin: None,
            margin_type: margin_type.clone(),
            updated_at: chrono::Utc::now(),
        });

        /**
         * update position quantity and entry price
         */
        let new_quantity = if position.side == side {
            position.quantity.clone() + quantity.clone()
        } else {
            if quantity > &position.quantity {
                position.side = side;
                quantity.clone() - position.quantity.clone()
            } else {
                position.quantity.clone() - quantity.clone()
            }
        };

        let new_entry_price = if position.side == side {
            let total_value = position.quantity.clone() * position.entry_price.clone() 
                + quantity.clone() * entry_price.clone();
            total_value / (position.quantity.clone() + quantity.clone())
        } else {
            if quantity > &position.quantity {
                entry_price.clone()
            } else {
                position.entry_price.clone()
            }
        };

        position.quantity = new_quantity;
        position.entry_price = new_entry_price.clone();
        position.updated_at = chrono::Utc::now();

        /**
         * update margin-related fields if it's a margin position
         */
        if position_type == PositionType::Margin {
            if let (Some(leverage), Some(margin_type)) = (&leverage, &margin_type) {
                position.leverage = Some(leverage.clone());
                position.margin_type = Some(*margin_type);
                
                if position.quantity > BigDecimal::from(0) {
                    position.liquidation_price = Some(MarginCalculator::calculate_liquidation_price(
                        &position.entry_price,
                        position.side,
                        leverage,
                        *margin_type,
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn check_margin_requirements(
        &self,
        order: &Order,
        current_price: &BigDecimal,
        margin_type: Option<MarginType>,
    ) -> Result<(), OrderError> {
        // skip margin checks for non-leveraged orders
        if order.leverage.is_none() || margin_type.is_none() {
            return Ok(());
        }

        let leverage = order.leverage.as_ref().unwrap();
        let margin_type = margin_type.unwrap();
        
        let required_margin = MarginCalculator::calculate_required_margin(
            &order.quantity,
            &order.price,
            leverage,
            margin_type,
        );

        // Check if account has enough balance
        let balance = self.get_balance("USDT"); // Assuming USDT margined
        if balance < required_margin {
            return Err(OrderError::InsufficientBalance);
        }

        // Check if position would be liquidated
        if let Some(position) = self.positions.get(&order.symbol) {
            if position.position_type == PositionType::Margin {
                let new_quantity = if position.side == order.side {
                    position.quantity.clone() + order.quantity.clone()
                } else {
                    if order.quantity > position.quantity {
                        order.quantity.clone() - position.quantity.clone()
                    } else {
                        position.quantity.clone() - order.quantity.clone()
                    }
                };

                let new_entry_price = if order.side == position.side {
                    (position.quantity.clone() * position.entry_price.clone() 
                        + order.quantity.clone() * order.price.clone()) 
                    / (position.quantity.clone() + order.quantity.clone())
                } else {
                    if order.quantity >= position.quantity {
                        order.price.clone()
                    } else {
                        position.entry_price.clone()
                    }
                };

                if MarginCalculator::is_position_liquidated(
                    current_price,
                    &new_entry_price,
                    order.side,
                    leverage,
                    margin_type,
                ) {
                    return Err(OrderError::WouldLiquidate);
                }
            }
        }

        Ok(())
    }
}