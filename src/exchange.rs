use crate::models::{Order, Trade, OrderError, FundingRate, Side, PositionType, MarginType, OrderBook, Account};
use crate::funding::FundingCalculator;
use crate::margin::MarginCalculator;
use bigdecimal::BigDecimal;
use uuid::Uuid;
use std::collections::HashMap;
use chrono::{Duration, Utc};

/**
 * exchange module implementation
 * handles order matching, trade execution, and position management
 * supports margin trading with configurable quote assets
 */

#[derive(Debug, Clone)]
pub struct MarketData {
    pub symbol: String,
    pub mark_price: BigDecimal,
    pub index_price: BigDecimal,
    pub open_interest_long: BigDecimal,
    pub open_interest_short: BigDecimal,
    pub last_update: chrono::DateTime<Utc>,
}

pub struct Exchange {
    pub accounts: HashMap<Uuid, Account>,
    pub order_books: HashMap<String, OrderBook>,
    pub funding_calculator: FundingCalculator,
    pub symbols: Vec<String>,
    pub market_data: HashMap<String, MarketData>,
    pub last_trade_prices: HashMap<String, BigDecimal>,
    pub quote_asset: String,
}

impl Exchange {
    pub fn new(symbols: Vec<String>, funding_interval: Duration, quote_asset: String) -> Self {
        let mut order_books = HashMap::new();
        let mut market_data = HashMap::new();
        let mut last_trade_prices = HashMap::new();

        for symbol in &symbols {
            order_books.insert(symbol.clone(), OrderBook::new(symbol.clone()));
            market_data.insert(symbol.clone(), MarketData {
                symbol: symbol.clone(),
                mark_price: BigDecimal::from(0),
                index_price: BigDecimal::from(0),
                open_interest_long: BigDecimal::from(0),
                open_interest_short: BigDecimal::from(0),
                last_update: Utc::now(),
            });
            last_trade_prices.insert(symbol.clone(), BigDecimal::from(0));
        }

        Exchange {
            accounts: HashMap::new(),
            order_books,
            funding_calculator: FundingCalculator::new(funding_interval),
            symbols,
            market_data,
            last_trade_prices,
            quote_asset,
        }
    }

    pub fn create_account(&mut self, user_id: Uuid) -> &mut Account {
        self.accounts.entry(user_id)
            .or_insert_with(|| Account::new(user_id))
    }

    pub fn get_account(&mut self, user_id: Uuid) -> Result<&mut Account, OrderError> {
        self.accounts.get_mut(&user_id)
            .ok_or(OrderError::OrderNotFound)
    }

    pub fn update_market_data(
        &mut self,
        symbol: &str,
        mark_price: BigDecimal,
        index_price: BigDecimal,
        open_interest_long: BigDecimal,
        open_interest_short: BigDecimal,
    ) {
        if let Some(market_data) = self.market_data.get_mut(symbol) {
            market_data.mark_price = mark_price;
            market_data.index_price = index_price;
            market_data.open_interest_long = open_interest_long;
            market_data.open_interest_short = open_interest_short;
            market_data.last_update = Utc::now();
        }
    }

    pub fn place_order(&mut self, order: Order) -> Result<Vec<Trade>, OrderError> {
        if !self.symbols.contains(&order.symbol) {
            return Err(OrderError::InvalidOrder);
        }

        let market_data = self.market_data.get(&order.symbol)
            .ok_or(OrderError::InvalidOrder)?
            .clone();

        if Utc::now() - market_data.last_update > chrono::Duration::seconds(30) {
            return Err(OrderError::InvalidOrder);
        }

        let quote_asset = self.quote_asset.clone();
        let account = self.get_account(order.user_id)?;

        account.check_margin_requirements(
            &order,
            &market_data.mark_price,
            Some(MarginType::Isolated),
        )?;

        if let Some(leverage) = &order.leverage {
            let required_margin = MarginCalculator::calculate_required_margin(
                &order.quantity,
                &order.price,
                leverage,
                MarginType::Isolated,
            );
            let balance = account.withdraw(quote_asset);
            if balance < required_margin {
                return Err(OrderError::InsufficientBalance);
            }
        }

        let order_book = self.order_books.get_mut(&order.symbol).unwrap();
        let trades = order_book.add_order(order)?;

        for trade in &trades {
            self.process_trade(trade)?;
            self.last_trade_prices.insert(trade.symbol.clone(), trade.price.clone());
        }

        Ok(trades)
    }

    fn process_trade(&mut self, trade: &Trade) -> Result<(), OrderError> {
        let order_book = self.order_books.get(&trade.symbol).unwrap();
        let buyer_order = order_book.bids.iter()
            .find(|o| o.id == trade.buyer_order_id)
            .ok_or(OrderError::OrderNotFound)?
            .clone();
        let seller_order = order_book.asks.iter()
            .find(|o| o.id == trade.seller_order_id)
            .ok_or(OrderError::OrderNotFound)?
            .clone();

        {
            let buyer_account = self.get_account(buyer_order.user_id)?;
            buyer_account.update_position(
                trade.symbol.clone(),
                Side::Buy,
                &trade.quantity,
                &trade.price,
                PositionType::Margin,
                &buyer_order.leverage,
                &Some(MarginType::Isolated),
            )?;
            if let Some(position) = buyer_account.positions.get_mut(&trade.symbol) {
                if position.quantity > BigDecimal::from(0) {
                    let pnl = (trade.price.clone() - position.entry_price.clone()) * position.quantity.clone();
                    if let Some(margin) = &mut position.margin {
                        *margin = margin.clone() + pnl;
                    }
                }
            }
        }

        {
            let seller_account = self.get_account(seller_order.user_id)?;
            seller_account.update_position(
                trade.symbol.clone(),
                Side::Sell,
                &trade.quantity,
                &trade.price,
                PositionType::Margin,
                &seller_order.leverage,
                &Some(MarginType::Isolated),
            )?;
            if let Some(position) = seller_account.positions.get_mut(&trade.symbol) {
                if position.quantity > BigDecimal::from(0) {
                    let pnl = (position.entry_price.clone() - trade.price.clone()) * position.quantity.clone();
                    if let Some(margin) = &mut position.margin {
                        *margin = margin.clone() + pnl;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn cancel_order(
        &mut self,
        user_id: Uuid,
        symbol: String,
        order_id: Uuid,
        side: Side,
    ) -> Result<(), OrderError> {
        let quote_asset = self.quote_asset.clone();
        let order_book = self.order_books.get_mut(&symbol)
            .ok_or(OrderError::InvalidOrder)?;

        let order = match side {
            Side::Buy => order_book.bids.iter().find(|o| o.id == order_id),
            Side::Sell => order_book.asks.iter().find(|o| o.id == order_id),
        }.ok_or(OrderError::OrderNotFound)?
        .clone();

        order_book.cancel_order(order_id, side)?;

        if let Some(leverage) = &order.leverage {
            let account = self.get_account(user_id)?;
            let required_margin = MarginCalculator::calculate_required_margin(
                &order.quantity,
                &order.price,
                leverage,
                MarginType::Isolated,
            );
            account.deposit(quote_asset, required_margin);
        }

        Ok(())
    }

    pub fn run_funding(&mut self) -> Result<Vec<FundingRate>, OrderError> {
        let mut new_rates = Vec::new();

        for symbol in &self.symbols {
            let market_data = self.market_data.get(symbol)
                .ok_or(OrderError::InvalidOrder)?;

            if Utc::now() - market_data.last_update > chrono::Duration::seconds(30) {
                return Err(OrderError::InvalidOrder);
            }

            let rate = self.funding_calculator.calculate_funding_rate(
                symbol.clone(),
                &market_data.mark_price,
                &market_data.index_price,
                &market_data.open_interest_long,
                &market_data.open_interest_short,
            );

            for account in self.accounts.values_mut() {
                self.funding_calculator.apply_funding(&mut account.positions, &rate)?;
            }

            new_rates.push(rate);
        }

        Ok(new_rates)
    }

    pub fn get_market_data(&self, symbol: &str) -> Option<&MarketData> {
        self.market_data.get(symbol)
    }

    pub fn get_last_trade_price(&self, symbol: &str) -> Option<&BigDecimal> {
        self.last_trade_prices.get(symbol)
    }
}