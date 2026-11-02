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

/// Maximum number of US stocks a user can hold in their watchlist.
pub const MAX_US_STOCKS: usize = 20;

/// Top US stocks known by symbol and company name.
pub const KNOWN_US_STOCKS: [(&str, &str); 40] = [
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
    ("INTC", "Intel Corporation"),
    ("BABA", "Alibaba Group"),
    ("DIS", "The Walt Disney Co."),
    ("UBER", "Uber Technologies"),
    ("PYPL", "PayPal Holdings"),
    ("CRM", "Salesforce Inc."),
    ("ORCL", "Oracle Corporation"),
    ("ADBE", "Adobe Inc."),
    ("NKE", "NIKE Inc."),
    ("SBUX", "Starbucks Corporation"),
    ("JPM", "JPMorgan Chase"),
    ("V", "Visa Inc."),
    ("MA", "Mastercard Inc."),
    ("BRK.B", "Berkshire Hathaway"),
    ("AVGO", "Broadcom Inc."),
    ("TSM", "Taiwan Semiconductor"),
    ("SMCI", "Super Micro Computer"),
    ("ARM", "Arm Holdings"),
    ("RDDT", "Reddit Inc."),
    ("DJT", "Trump Media & Tech"),
    ("SOUN", "SoundHound AI"),
    ("MSTR", "MicroStrategy Inc."),
    ("MCD", "McDonald's Corp."),
    ("KO", "The Coca-Cola Co."),
    ("WMT", "Walmart Inc."),
    ("COST", "Costco Wholesale"),
    ("VOO", "Vanguard S&P 500 ETF"),
];

pub fn name_for(sym: &str) -> &'static str {
    for &(s, name) in &KNOWN_US_STOCKS {
        if s.eq_ignore_ascii_case(sym) {
            return name;
        }
    }
    "US Stock"
}

/// The user's persisted US Stocks watchlist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UsStockWatchlist {
    pub codes: Vec<String>,
    pub pick: String,
    #[serde(default)]
    pub show_pct: bool,
    #[serde(default)]
    pub range: crate::stocks::Range,
}

impl Default for UsStockWatchlist {
    fn default() -> Self {
        Self {
            codes: STARTERS.iter().map(|&(sym, _)| sym.to_string()).collect(),
            pick: "AAPL".to_string(),
            show_pct: true,
            range: crate::stocks::Range::Day,
        }
    }
}

impl UsStockWatchlist {
    pub fn full(&self) -> bool {
        self.codes.len() >= MAX_US_STOCKS
    }

    pub fn has(&self, code: &str) -> bool {
        self.codes.iter().any(|c| c.eq_ignore_ascii_case(code))
    }

    pub fn add(&mut self, code: &str) -> bool {
        let code = code.trim().to_ascii_uppercase();
        if self.full() || self.has(&code) || code.is_empty() {
            return false;
        }
        self.codes.push(code);
        true
    }

    pub fn remove(&mut self, code: &str) {
        if let Some(pos) = self.codes.iter().position(|c| c.eq_ignore_ascii_case(code)) {
            self.codes.remove(pos);
            if self.pick.eq_ignore_ascii_case(code) {
                self.pick = self.codes.first().cloned().unwrap_or_else(|| "AAPL".to_string());
            }
        }
    }

    pub fn move_to(&mut self, code: &str, target_idx: usize) {
        if let Some(cur) = self.codes.iter().position(|c| c.eq_ignore_ascii_case(code)) {
            let item = self.codes.remove(cur);
            let to = target_idx.min(self.codes.len());
            self.codes.insert(to, item);
        }
    }
}

/// Autocomplete and search helper over known US stocks.
pub fn search_us_stocks(query: &str, skip: &[String]) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_ascii_uppercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, usize, &'static str, &'static str)> = Vec::new();
    for (idx, &(c, n)) in KNOWN_US_STOCKS.iter().enumerate() {
        if skip.iter().any(|x| x.eq_ignore_ascii_case(c)) {
            continue;
        }
        let nu = n.to_ascii_uppercase();
        let rank = if c == q {
            0
        } else if c.starts_with(&q) {
            1
        } else if nu == q {
            2
        } else if nu.starts_with(&q) {
            3
        } else if nu.split_whitespace().any(|w| w.starts_with(&q)) {
            4
        } else if c.contains(&q) {
            5
        } else if nu.contains(&q) {
            6
        } else {
            continue;
        };
        ranked.push((rank, idx, c, n));
    }
    ranked.sort_by_key(|&(rank, idx, _, _)| (rank, idx));
    ranked.into_iter().map(|(_, _, c, n)| (c, n)).collect()
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
    /// The 52-week (low, high) as Yahoo reports it; None when it doesn't.
    pub w52: Option<(f64, f64)>,
    /// Day sparkline points: (minute since the 9:30 ET opening bell, price).
    pub spark: Vec<(u16, f32)>,
    /// The session's first traded price.
    #[serde(default)]
    pub open: Option<f64>,
}

/// Minutes in a regular NYSE/NASDAQ session (9:30 to 16:00 ET).
pub const SESSION_MIN: u16 = 390;

/// Where minute `m` of the session sits across a day chart, 0..1.
pub fn day_x(m: u16) -> f32 {
    f32::from(m.min(SESSION_MIN)) / f32::from(SESSION_MIN)
}

/// Minute `m` of the session as a New York clock time, like "10:35 AM".
pub fn session_label(m: u16) -> String {
    let t = 9 * 60 + 30 + u32::from(m.min(SESSION_MIN));
    let (h, mm) = (t / 60, t % 60);
    let (h12, ap) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{h12}:{mm:02} {ap}")
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
            w52: None,
            spark,
            open: None,
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

/// One answer of Yahoo Finance's `/v8/finance/chart/{ticker}?range=1d`.
#[derive(Clone, Debug, PartialEq)]
pub struct YahooChart {
    pub price: f64,
    pub prev: f64,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub volume: Option<f64>,
    pub w52: Option<(f64, f64)>,
    pub short_name: Option<String>,
    /// (minute since the opening bell, close), in time order.
    pub spark: Vec<(u16, f32)>,
}

fn pos(v: Option<&serde_json::Value>) -> Option<f64> {
    v.and_then(serde_json::Value::as_f64).filter(|x| x.is_finite() && *x > 0.0)
}

/// Reads a day chart. Each point sits at its real time in the session
/// (from its timestamp and the session's start), so a chart drawn before
/// the close stops where trading has got to, as the IDX page's does.
pub fn parse_yahoo_chart_json(json_str: &str) -> Result<YahooChart, String> {
    let v: serde_json::Value = serde_json::from_str(json_str).map_err(|e| format!("parse error: {e}"))?;
    let res = v.get("chart").and_then(|c| c.get("result")).and_then(|r| r.as_array()).and_then(|a| a.first()).ok_or("no chart.result[0]")?;
    let meta = res.get("meta").ok_or("no meta")?;

    let price = pos(meta.get("regularMarketPrice")).ok_or("no price")?;
    let prev = pos(meta.get("chartPreviousClose")).or_else(|| pos(meta.get("previousClose"))).unwrap_or(price);
    let high = pos(meta.get("regularMarketDayHigh"));
    let low = pos(meta.get("regularMarketDayLow"));
    let volume = pos(meta.get("regularMarketVolume"));
    let w52 = pos(meta.get("fiftyTwoWeekLow")).zip(pos(meta.get("fiftyTwoWeekHigh")));
    let short_name = meta.get("shortName").or_else(|| meta.get("longName")).and_then(|s| s.as_str()).map(|s| s.to_string());
    let start = meta.get("currentTradingPeriod").and_then(|p| p.get("regular")).and_then(|r| r.get("start")).and_then(serde_json::Value::as_i64);

    let mut spark = Vec::new();
    let mut open = None;
    let ts = res.get("timestamp").and_then(|t| t.as_array());
    let quote = res.get("indicators").and_then(|i| i.get("quote")).and_then(|q| q.as_array()).and_then(|a| a.first());
    if let (Some(ts), Some(quote)) = (ts, quote) {
        open = quote.get("open").and_then(|o| o.as_array()).and_then(|o| o.iter().find_map(|x| pos(Some(x))));
        let start = start.or_else(|| ts.first().and_then(serde_json::Value::as_i64)).unwrap_or(0);
        if let Some(closes) = quote.get("close").and_then(|c| c.as_array()) {
            for (t, c) in ts.iter().zip(closes) {
                if let (Some(t), Some(c)) = (t.as_i64(), pos(Some(c))) {
                    if t < start {
                        continue;
                    }
                    let m = ((t - start) / 60).clamp(0, i64::from(SESSION_MIN)) as u16;
                    spark.push((m, c as f32));
                }
            }
        }
    }
    spark.sort_by_key(|p| p.0);
    spark.dedup_by_key(|p| p.0);
    // The line ends on the price shown above it.
    if let Some(last) = spark.last_mut() {
        last.1 = price as f32;
    }

    Ok(YahooChart { price, prev, open, high, low, volume, w52, short_name, spark })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_chart_sits_each_point_at_its_time_in_the_session() {
        // Session opens at 1_700_000_000; points at the open, +5 min and +60 min.
        let json = r#"{"chart":{"result":[{"meta":{"regularMarketPrice":101.5,"chartPreviousClose":100.0,
            "regularMarketDayHigh":102.0,"regularMarketDayLow":99.5,"regularMarketVolume":1000,
            "fiftyTwoWeekLow":80.0,"fiftyTwoWeekHigh":120.0,
            "currentTradingPeriod":{"regular":{"start":1700000000,"end":1700023400}}},
            "timestamp":[1700000000,1700000300,1700003600],
            "indicators":{"quote":[{"open":[null,100.2,100.9],"close":[100.1,null,101.0]}]}}]}}"#;
        let c = parse_yahoo_chart_json(json).unwrap();
        assert_eq!(c.prev, 100.0);
        assert_eq!(c.open, Some(100.2));
        assert_eq!(c.w52, Some((80.0, 120.0)));
        // The null close is skipped; the last point ends on the price.
        assert_eq!(c.spark, vec![(0, 100.1), (60, 101.5)]);
    }

    #[test]
    fn session_minutes_read_as_new_york_clock_times() {
        assert_eq!(session_label(0), "9:30 AM");
        assert_eq!(session_label(150), "12:00 PM");
        assert_eq!(session_label(SESSION_MIN), "4:00 PM");
        assert_eq!(day_x(195), 0.5);
    }

    #[test]
    fn us_stock_watchlist_add_remove_search() {
        let mut wl = UsStockWatchlist::default();
        assert!(wl.has("AAPL"));
        assert!(!wl.has("CRWD"));
        assert!(wl.add("CRWD"));
        assert!(wl.has("CRWD"));
        wl.remove("CRWD");
        assert!(!wl.has("CRWD"));

        let hits = search_us_stocks("TES", &[]);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0, "TSLA");
    }
}
