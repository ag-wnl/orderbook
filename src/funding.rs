use crate::models::{FundingRate, OrderError, Position, Side, PositionType};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct FundingPayment {
    pub symbol: String,
    pub rate: BigDecimal,
    pub payment: BigDecimal,
    pub timestamp: DateTime<Utc>,
}

pub struct FundingCalculator {
    funding_interval: Duration,
    funding_rate_history: Vec<FundingRate>,
    funding_payments: Vec<FundingPayment>,
    last_funding_time: DateTime<Utc>,
}

impl FundingCalculator {
    /**
     * creates a new funding calculator with specified interval
     */
    pub fn new(funding_interval: Duration) -> Self {
        FundingCalculator {
            funding_interval,
            funding_rate_history: Vec::new(),
            funding_payments: Vec::new(),
            last_funding_time: Utc::now(),
        }
    }

    /**
     * calculates funding rate based on mark price, index price, and open interest
     * applies premium rate, base interest rate, and open interest impact
     * clamps rate to typical bounds of ±0.075%
     */
    pub fn calculate_funding_rate(
        &mut self,
        symbol: String,
        mark_price: &BigDecimal,
        index_price: &BigDecimal,
        open_interest_long: &BigDecimal,
        open_interest_short: &BigDecimal,
    ) -> FundingRate {
        let price_diff = mark_price - index_price;
        let premium_rate = price_diff / index_price.clone();
        let base_interest_rate = BigDecimal::from_str("0.0001").unwrap();
        let total_oi = open_interest_long + open_interest_short;
        let oi_ratio = if total_oi > BigDecimal::from(0) {
            (open_interest_long - open_interest_short) / total_oi
        } else {
            BigDecimal::from(0)
        };
        let oi_impact = oi_ratio * BigDecimal::from_str("0.0001").unwrap();
        let funding_rate = premium_rate + base_interest_rate + oi_impact;
        let clamped_rate = funding_rate.max(BigDecimal::from_str("-0.00075").unwrap())
            .min(BigDecimal::from_str("0.00075").unwrap());
        let next_funding_time = self.last_funding_time + self.funding_interval;
        let funding_rate = FundingRate {
            symbol,
            rate: clamped_rate.clone(),
            next_funding_time,
        };
        self.funding_rate_history.push(funding_rate.clone());
        self.last_funding_time = next_funding_time;
        funding_rate
    }

    /**
     * applies funding payments to positions
     * updates position margins based on funding rate
     * records funding payments in history
     */
    pub fn apply_funding(
        &mut self,
        positions: &mut HashMap<String, Position>,
        funding_rate: &FundingRate,
    ) -> Result<(), OrderError> {
        let current_time = Utc::now();
        if current_time < funding_rate.next_funding_time {
            return Ok(());
        }

        for position in positions.values_mut() {
            if position.symbol != funding_rate.symbol 
                || position.quantity == BigDecimal::from(0)
                || position.position_type != PositionType::Margin {
                continue;
            }

            let position_value = position.quantity.clone() * position.entry_price.clone();
            let funding_payment = position_value * funding_rate.rate.clone();

            let payment = FundingPayment {
                symbol: position.symbol.clone(),
                rate: funding_rate.rate.clone(),
                payment: funding_payment.clone(),
                timestamp: current_time,
            };
            self.funding_payments.push(payment);

            if let Some(margin) = &mut position.margin {
                match position.side {
                    Side::Buy => {
                        *margin = margin.clone() - funding_payment;
                    }
                    Side::Sell => {
                        *margin = margin.clone() + funding_payment;
                    }
                }

                if margin.clone() < BigDecimal::from(0) {
                    return Err(OrderError::FundingError);
                }
            }
        }

        Ok(())
    }

    /**
     * returns the history of funding rates
     */
    pub fn get_funding_history(&self) -> &[FundingRate] {
        &self.funding_rate_history
    }

    /**
     * returns the history of funding payments
     */
    pub fn get_funding_payments(&self) -> &[FundingPayment] {
        &self.funding_payments
    }
}