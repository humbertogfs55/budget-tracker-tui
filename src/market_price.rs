//! BTC/BRL prices from Binance's public market data (no key needed). Minute candles go back
//! years, so a trade can be priced at the minute it happened.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use std::time::Duration;

const API: &str = "https://api.binance.com/api/v3";
const SYMBOL: &str = "BTCBRL";

pub struct BtcPrices {
    agent: ureq::Agent,
}

#[derive(Deserialize)]
struct Ticker {
    price: String,
}

impl BtcPrices {
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(15)))
                .build()
                .into(),
        }
    }

    pub fn now(&self) -> Result<Decimal, String> {
        let ticker: Ticker = self
            .agent
            .get(format!("{}/ticker/price?symbol={}", API, SYMBOL))
            .call()
            .and_then(|mut response| response.body_mut().read_json())
            .map_err(|err| format!("BTC price: {}", err))?;
        parse(&ticker.price)
    }

    /// Midpoint of the one-minute candle containing `instant`.
    pub fn at(&self, instant: DateTime<Utc>) -> Result<Decimal, String> {
        let minute_start = instant.timestamp() / 60 * 60 * 1000;
        let candles: Vec<Vec<serde_json::Value>> = self
            .agent
            .get(format!(
                "{}/klines?symbol={}&interval=1m&startTime={}&limit=1",
                API, SYMBOL, minute_start
            ))
            .call()
            .and_then(|mut response| response.body_mut().read_json())
            .map_err(|err| format!("BTC price history: {}", err))?;
        let candle = candles
            .first()
            .ok_or_else(|| format!("no BTC price for {}", instant))?;
        // Kline layout: [open time, open, high, low, close, ...], prices as strings.
        let field = |index: usize| {
            candle
                .get(index)
                .and_then(|value| value.as_str())
                .ok_or_else(|| "unexpected BTC price format".to_string())
                .and_then(parse)
        };
        Ok((field(2)? + field(3)?) / Decimal::TWO)
    }
}

fn parse(value: &str) -> Result<Decimal, String> {
    Decimal::from_str(value).map_err(|err| format!("BTC price '{}': {}", value, err))
}
