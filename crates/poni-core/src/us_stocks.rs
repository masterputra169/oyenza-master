//! Real-time US Stocks models, presets and Yahoo Finance parsers.
//!
//! Complies with the repository rule:
//! - Free, zero-API-key, lightweight endpoints (Yahoo Finance `/v8/finance/chart`).
//! - Standard serde JSON deserialization.

use crate::stocks::Trend;
use serde::{Deserialize, Serialize};

/// Starter US stocks preset with ticker, full name, and sample quote data.
pub const STARTERS: [(&str, &str); 10] = [
    ("AAPL", "Apple Inc."),
    ("NVDA", "NVIDIA Corporation"),
    ("MSFT", "Microsoft Corporation"),
    ("TSLA", "Tesla Inc."),
    ("GOOGL", "Alphabet Inc."),
    ("AMZN", "Amazon.com Inc."),
    ("META", "Meta Platforms Inc."),
    ("AMD", "Advanced Micro Devices"),
    ("NFLX", "Netflix Inc."),
    ("PLTR", "Palantir Technologies"),
];

/// Top US stocks known by symbol and company name.
pub const KNOWN_US_STOCKS: [(&str, &str); 13] = [
    ("AAPL", "Apple Inc."),
    ("NVDA", "NVIDIA Corporation"),
    ("MSFT", "Microsoft Corporation"),
    ("TSLA", "Tesla Inc."),
    ("GOOGL", "Alphabet Inc."),
    ("AMZN", "Amazon.com Inc."),
    ("META", "Meta Platforms Inc."),
    ("AMD", "Advanced Micro Devices"),
    ("NFLX", "Netflix Inc."),
    ("PLTR", "Palantir Technologies"),
    ("COIN", "Coinbase Global"),
    ("SPY", "SPDR S&P 500 ETF"),
    ("QQQ", "Invesco QQQ Trust"),
];

pub fn name_for(sym: &str) -> &'static str {
    for &(s, name) in &KNOWN_US_STOCKS {
        if s.eq_ignore_ascii_case(sym) {
            return name;
        }
    }
    "US Stock"
}

/// One US stock quote row: symbol, company name, latest price, day range and sparkline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsStockQuote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub prev: f64,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<f64>,
    pub w52: Option<(f64, f64)>,
    /// Day sparkline points: (relative minute, price)
    pub spark: Vec<(u16, f32)>,
}

impl UsStockQuote {
    pub fn new(symbol: &str, name: &str, price: f64, prev: f64, high: f64, low: f64, vol: f64, spark: Vec<(u16, f32)>) -> Self {
        Self {
            symbol: symbol.to_string(),
            name: name.to_string(),
            price,
            prev,
            high: Some(high),
            low: Some(low),
            volume: Some(vol),
            w52: Some((low * 0.85, high * 1.15)),
            spark,
        }
    }

    pub fn pct(&self) -> f64 {
        if self.prev <= 0.0 {
            0.0
        } else {
            ((self.price - self.prev) / self.prev) * 100.0
        }
    }

    pub fn diff(&self) -> f64 {
        self.price - self.prev
    }

    pub fn trend(&self) -> Trend {
        let diff = self.diff();
        if diff > 0.001 {
            Trend::Up
        } else if diff < -0.001 {
            Trend::Down
        } else {
            Trend::Flat
        }
    }

    pub fn fmt_price(&self) -> String {
        format!("${:.2}", self.price)
    }

    pub fn fmt_pct(&self) -> String {
        let p = self.pct();
        if p >= 0.0 {
            format!("+{:.2}%", p)
        } else {
            format!("{:.2}%", p)
        }
    }
}

/// A snapshot of all watched US stocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsStockBoard {
    pub quotes: Vec<UsStockQuote>,
    pub updated_at: u64,
}

impl UsStockBoard {
    pub fn find(&self, symbol: &str) -> Option<&UsStockQuote> {
        self.quotes.iter().find(|q| q.symbol.eq_ignore_ascii_case(symbol))
    }

    pub fn default_board() -> Self {
        let spark_up: Vec<(u16, f32)> = (0..20)
            .map(|i| {
                let m = (i * 20) as u16;
                let v = 100.0 + (i as f32 * 0.8) + ((i % 3) as f32 * 0.5 - 0.25);
                (m, v)
            })
            .collect();

        let spark_down: Vec<(u16, f32)> = (0..20)
            .map(|i| {
                let m = (i * 20) as u16;
                let v = 100.0 - (i as f32 * 0.7) + ((i % 2) as f32 * 0.4 - 0.2);
                (m, v)
            })
            .collect();

        let quotes = vec![
            UsStockQuote::new("AAPL", "Apple Inc.", 344.63, 348.91, 349.19, 342.35, 67_840_000.0, spark_down.clone()),
            UsStockQuote::new("NVDA", "NVIDIA Corporation", 141.54, 140.20, 142.80, 139.10, 52_130_000.0, spark_up.clone()),
            UsStockQuote::new("MSFT", "Microsoft Corporation", 432.53, 430.12, 434.90, 428.60, 21_400_000.0, spark_up.clone()),
            UsStockQuote::new("TSLA", "Tesla Inc.", 269.19, 262.50, 273.40, 261.20, 89_320_000.0, spark_up.clone()),
            UsStockQuote::new("GOOGL", "Alphabet Inc.", 181.22, 180.45, 182.90, 179.80, 25_100_000.0, spark_up.clone()),
            UsStockQuote::new("AMZN", "Amazon.com Inc.", 197.93, 196.40, 199.10, 195.20, 31_200_000.0, spark_up.clone()),
            UsStockQuote::new("META", "Meta Platforms Inc.", 591.80, 595.20, 598.40, 588.60, 14_900_000.0, spark_down.clone()),
            UsStockQuote::new("AMD", "Advanced Micro Devices", 156.40, 154.10, 158.20, 153.50, 42_700_000.0, spark_up.clone()),
            UsStockQuote::new("NFLX", "Netflix Inc.", 760.30, 755.80, 764.50, 751.20, 3_800_000.0, spark_up.clone()),
            UsStockQuote::new("PLTR", "Palantir Technologies", 45.20, 43.80, 46.10, 43.50, 68_100_000.0, spark_up),
        ];

        Self {
            quotes,
            updated_at: 0,
        }
    }
}

/// Parses Yahoo Finance `/v8/finance/chart/{ticker}` JSON into (price, prev_close, high, low, volume, sparkline).
pub fn parse_yahoo_chart_json(json_str: &str) -> Result<(f64, f64, Option<f64>, Option<f64>, Option<f64>, Vec<(u16, f32)>), String> {
    let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("parse error: {e}"))?;
    let res = v.get("chart").and_then(|c| c.get("result")).and_then(|r| r.as_array()).and_then(|a| a.first()).ok_or("no chart.result[0]")?;
    let meta = res.get("meta").ok_or("no meta")?;

    let price = meta.get("regularMarketPrice").and_then(|p| p.as_f64()).unwrap_or(0.0);
    let prev = meta.get("chartPreviousClose").or_else(|| meta.get("previousClose")).and_then(|p| p.as_f64()).unwrap_or(price);
    let high = meta.get("regularMarketDayHigh").and_then(|p| p.as_f64());
    let low = meta.get("regularMarketDayLow").and_then(|p| p.as_f64());
    let volume = meta.get("regularMarketVolume").and_then(|p| p.as_f64());

    let mut spark = Vec::new();
    if let Some(indicators) = res.get("indicators") {
        if let Some(quote_arr) = indicators.get("quote").and_then(|q| q.as_array()).and_then(|a| a.first()) {
            if let Some(closes) = quote_arr.get("close").and_then(|c| c.as_array()) {
                let count = closes.len();
                for (i, val) in closes.iter().enumerate() {
                    if let Some(c) = val.as_f64().filter(|c| *c > 0.0) {
                        let min = if count > 0 { ((i as f32 / count as f32) * 390.0) as u16 } else { 0 };
                        spark.push((min, c as f32));
                    }
                }
            }
        }
    }

    Ok((price, prev, high, low, volume, spark))
}

/// Parses Yahoo Finance daily historical closes from `/v8/finance/chart/{ticker}?interval=1d&range=1y`.
pub fn parse_yahoo_history_json(json_str: &str) -> Result<Vec<(i32, f64)>, String> {
    let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("parse error: {e}"))?;
    let res = v.get("chart").and_then(|c| c.get("result")).and_then(|r| r.as_array()).and_then(|a| a.first()).ok_or("no chart.result[0]")?;
    let timestamps = res.get("timestamp").and_then(|t| t.as_array()).ok_or("no timestamps")?;

    let closes = res
        .get("indicators")
        .and_then(|i| i.get("quote"))
        .and_then(|q| q.as_array())
        .and_then(|a| a.first())
        .and_then(|q| q.get("close"))
        .and_then(|c| c.as_array())
        .ok_or("no close quotes")?;

    let mut points = Vec::new();
    for (ts_val, close_val) in timestamps.iter().zip(closes.iter()) {
        if let (Some(ts), Some(c)) = (ts_val.as_i64(), close_val.as_f64()) {
            if c > 0.0 {
                // days since unix epoch (1970-01-01)
                let day = (ts / 86400) as i32;
                points.push((day, c));
            }
        }
    }

    Ok(points)
}
