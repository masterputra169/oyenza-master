//! Real-time Forex & Commodities models, watchlists, and calculations.
//!
//! Complies with the repository rule:
//! - Pure Rust in poni-core, zero CPU at rest.
//! - Free public endpoints (open.er-api.com, jsdelivr currency-api, Binance PAXG, Yahoo Finance).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Troy ounce to gram conversion factor.
pub const TROY_OUNCE_GRAMS: f64 = 31.1034768;
/// Antam physical gold manufacturing & tax premium over international spot.
pub const ANTAM_PREMIUM_FACTOR: f64 = 1.035;

/// Default watchlist order for Forex & Commodities.
pub const DEFAULT_SYMBOLS: &[&str] = &[
    "USD/IDR",
    "GOLD",
    "SILVER",
    "OIL",
    "SGD/IDR",
    "EUR/IDR",
    "JPY/IDR",
    "SAR/IDR",
    "MYR/IDR",
    "CNY/IDR",
    "AUD/IDR",
    "GBP/IDR",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForexQuote {
    pub symbol: String,
    pub name: String,
    pub rate: f64,
    pub open: f64,
    pub prev_close: f64,
    pub low: f64,
    pub high: f64,
    pub change_pct: f64,
    pub is_gold: bool,
    pub unit: String,
    #[serde(default)]
    pub spark: Vec<(u16, f32)>,
}

impl ForexQuote {
    pub fn trend(&self) -> i8 {
        poni_core_trend(self.change_pct)
    }

    pub fn is_usd_priced(&self) -> bool {
        matches!(self.symbol.as_str(), "OIL" | "MINYAK" | "WTI")
    }

    pub fn formatted_rate(&self) -> String {
        fmt_quote_val(&self.symbol, self.rate)
    }

    pub fn formatted_change(&self, show_pct: bool) -> String {
        if show_pct {
            let sign = if self.change_pct > 0.005 {
                "+"
            } else if self.change_pct < -0.005 {
                "\u{2212}"
            } else {
                ""
            };
            format!("{sign}{:.2}%", self.change_pct.abs())
        } else {
            let diff = self.rate - self.prev_close;
            let sign = if diff > 0.0 {
                "+"
            } else if diff < 0.0 {
                "\u{2212}"
            } else {
                ""
            };
            if self.is_usd_priced() {
                format!("{sign}${:.2}", diff.abs())
            } else {
                format!("{sign}{}", fmt_idr_plain(diff.abs()))
            }
        }
    }

    pub fn idle_text(&self) -> String {
        let sign = if self.change_pct >= 0.0 { "+" } else { "" };
        format!("{} {} {sign}{:.1}%", self.symbol, self.formatted_rate(), self.change_pct)
    }
}

pub fn fmt_quote_val(symbol: &str, val: f64) -> String {
    match symbol.to_ascii_uppercase().as_str() {
        "OIL" | "MINYAK" | "WTI" => format!("${:.2}", val),
        "GOLD" | "EMAS" | "XAU/IDR" => fmt_idr(val, true),
        "SILVER" | "PERAK" | "XAG/IDR" => format!("{} /g", fmt_idr(val, false)),
        _ => fmt_idr(val, false),
    }
}

pub fn fmt_idr_plain(val: f64) -> String {
    let rounded = val.round() as i64;
    let s = rounded.abs().to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.into_iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

fn poni_core_trend(pct: f64) -> i8 {
    if pct > 0.02 { 1 } else if pct < -0.02 { -1 } else { 0 }
}

/// Optional live commodity overrides from market feeds (Binance PAXG, Yahoo Finance GC=F, SI=F, CL=F).
#[derive(Clone, Debug, Default)]
pub struct CommodityFeeds {
    /// Gold spot USD/oz and 24h change %
    pub gold_usd: Option<(f64, f64)>,
    /// Silver spot USD/oz and 24h change %
    pub silver_usd: Option<(f64, f64)>,
    /// WTI Crude Oil USD/bbl and 24h change %
    pub oil_usd: Option<(f64, f64)>,
    /// Real intraday sparklines keyed by symbol
    pub sparks: HashMap<String, Vec<(u16, f32)>>,
    /// Real 24h change % overrides keyed by symbol
    pub pcts: HashMap<String, f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForexBoard {
    pub quotes: Vec<ForexQuote>,
    pub updated_at: u64,
}

impl ForexBoard {
    pub fn default_board() -> Self {
        let mut rates = HashMap::new();
        rates.insert("IDR".into(), 18132.0);
        rates.insert("SGD".into(), 1.2782);
        rates.insert("EUR".into(), 0.8956);
        rates.insert("JPY".into(), 157.49);
        rates.insert("SAR".into(), 3.7500);
        rates.insert("MYR".into(), 4.3850);
        rates.insert("CNY".into(), 7.1250);
        rates.insert("AUD".into(), 1.5180);
        rates.insert("GBP".into(), 0.7715);
        Self::from_rates(&rates, Some(3898.0), 0)
    }

    pub fn quote(&self, sym: &str) -> Option<&ForexQuote> {
        let upper = normalize_symbol(sym);
        self.quotes.iter().find(|q| q.symbol.eq_ignore_ascii_case(&upper))
    }

    pub fn from_rates(rates: &HashMap<String, f64>, paxg_usd: Option<f64>, now_unix: u64) -> Self {
        let feeds = CommodityFeeds {
            gold_usd: paxg_usd.map(|p| (p, -0.24)),
            ..Default::default()
        };
        Self::from_rates_and_feeds(rates, &feeds, now_unix)
    }

    pub fn from_rates_and_feeds(rates: &HashMap<String, f64>, feeds: &CommodityFeeds, now_unix: u64) -> Self {
        let idr = *rates.get("IDR").unwrap_or(&18132.0);
        let mut quotes = Vec::with_capacity(12);

        let build = |sym: &str, name: &str, rate: f64, def_pct: f64, is_gold: bool, unit: &str| -> ForexQuote {
            let pct = feeds.pcts.get(sym).copied().unwrap_or(def_pct);
            let mut q = make_quote(sym, name, rate, pct, is_gold, unit);
            if let Some(sp) = feeds.sparks.get(sym) {
                if sp.len() >= 2 {
                    let lo = sp.iter().map(|p| f64::from(p.1)).fold(q.low, f64::min);
                    let hi = sp.iter().map(|p| f64::from(p.1)).fold(q.high, f64::max);
                    q.low = lo;
                    q.high = hi;
                    q.open = f64::from(sp[0].1);
                    q.spark = sp.clone();
                }
            }
            q
        };

        // 1. USD/IDR
        quotes.push(build("USD/IDR", "US Dollar", idr, 0.12, false, "per 1 USD"));

        // 2. GOLD (Antam / g)
        let (gold_usd_oz, gold_pct) = feeds.gold_usd.unwrap_or((3898.9, -0.24));
        let gold_idr_gram = (gold_usd_oz * idr / TROY_OUNCE_GRAMS) * ANTAM_PREMIUM_FACTOR;
        quotes.push(build("GOLD", "Gold (Antam / g)", gold_idr_gram, gold_pct, true, "per 1 gram"));

        // 3. SILVER (Antam / g)
        let (silver_usd_oz, silver_pct) = feeds.silver_usd.unwrap_or((49.28, 0.45));
        let silver_idr_gram = (silver_usd_oz * idr / TROY_OUNCE_GRAMS) * 1.05;
        quotes.push(build("SILVER", "Silver (Spot / g)", silver_idr_gram, silver_pct, false, "per 1 gram"));

        // 4. OIL (WTI Crude Oil)
        let (oil_usd_barrel, oil_pct) = feeds.oil_usd.unwrap_or((74.50, -0.62));
        quotes.push(build("OIL", "WTI Crude Oil", oil_usd_barrel, oil_pct, false, "per barrel (USD)"));

        // 5. SGD/IDR
        let sgd = rates.get("SGD").copied().filter(|&v| v > 0.0).unwrap_or(1.2782);
        quotes.push(build("SGD/IDR", "Singapore Dollar", idr / sgd, 0.08, false, "per 1 SGD"));

        // 6. EUR/IDR
        let eur = rates.get("EUR").copied().filter(|&v| v > 0.0).unwrap_or(0.8956);
        quotes.push(build("EUR/IDR", "Euro", idr / eur, -0.15, false, "per 1 EUR"));

        // 7. JPY/IDR (per 100 JPY)
        let jpy = rates.get("JPY").copied().filter(|&v| v > 0.0).unwrap_or(157.49);
        quotes.push(build("JPY/IDR", "Japanese Yen (100)", (idr / jpy) * 100.0, 0.31, false, "per 100 JPY"));

        // 8. SAR/IDR (Saudi Riyal)
        let sar = rates.get("SAR").copied().filter(|&v| v > 0.0).unwrap_or(3.7500);
        quotes.push(build("SAR/IDR", "Saudi Riyal", idr / sar, 0.05, false, "per 1 SAR"));

        // 9. MYR/IDR
        let myr = rates.get("MYR").copied().filter(|&v| v > 0.0).unwrap_or(4.3850);
        quotes.push(build("MYR/IDR", "Malaysian Ringgit", idr / myr, 0.06, false, "per 1 MYR"));

        // 10. CNY/IDR
        let cny = rates.get("CNY").copied().filter(|&v| v > 0.0).unwrap_or(7.1250);
        quotes.push(build("CNY/IDR", "Chinese Yuan", idr / cny, -0.04, false, "per 1 CNY"));

        // 11. AUD/IDR
        let aud = rates.get("AUD").copied().filter(|&v| v > 0.0).unwrap_or(1.5180);
        quotes.push(build("AUD/IDR", "Australian Dollar", idr / aud, 0.18, false, "per 1 AUD"));

        // 12. GBP/IDR
        let gbp = rates.get("GBP").copied().filter(|&v| v > 0.0).unwrap_or(0.7715);
        quotes.push(build("GBP/IDR", "British Pound", idr / gbp, -0.11, false, "per 1 GBP"));

        Self { quotes, updated_at: now_unix }
    }
}

pub fn normalize_symbol(sym: &str) -> String {
    match sym.trim().to_ascii_uppercase().as_str() {
        "EMAS" | "XAU" | "XAU/IDR" | "GOLD" => "GOLD".into(),
        "PERAK" | "XAG" | "XAG/IDR" | "SILVER" => "SILVER".into(),
        "MINYAK" | "WTI" | "CL" | "OIL" => "OIL".into(),
        other => other.into(),
    }
}

fn make_quote(symbol: &str, name: &str, rate: f64, pct: f64, is_gold: bool, unit: &str) -> ForexQuote {
    let prev = rate / (1.0 + pct / 100.0);
    let spread = (rate * 0.0045).max(if rate < 500.0 { 0.35 } else { 15.0 });
    let low = (rate.min(prev) - spread).max(0.01);
    let high = rate.max(prev) + spread;
    let spark = synthetic_spark(symbol, prev, rate, low, high);
    ForexQuote {
        symbol: symbol.into(),
        name: name.into(),
        rate,
        open: prev,
        prev_close: prev,
        low,
        high,
        change_pct: pct,
        is_gold,
        unit: unit.into(),
        spark,
    }
}

/// Generates a realistic 48-point 24h intraday curve between `prev` and `price`.
pub fn synthetic_spark(code: &str, prev: f64, price: f64, low: f64, high: f64) -> Vec<(u16, f32)> {
    let mut seed = code.bytes().fold(0x9E37_79B9u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut spark = Vec::with_capacity(48);
    let swing = (high - low).max((price - prev).abs()).max(prev * 0.005);
    let mut lean = 0.0;
    let steps = 47u16;
    for k in 0..=steps {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        lean = lean * 0.80 + (f64::from(seed % 1000) / 1000.0 - 0.5) * swing * 0.35;
        let f = f64::from(k) / f64::from(steps);
        let p = if k == 0 {
            prev
        } else if k == steps {
            price
        } else {
            (prev + (price - prev) * f + lean * (1.0 - (2.0 * f - 1.0).powi(2))).clamp(low, high)
        };
        spark.push((k * 30, p as f32));
    }
    spark
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForexHistory {
    pub symbol: String,
    pub closes: Vec<(i32, f64)>,
}

pub fn sample_history(symbol: &str, today: i32, rate: f64) -> ForexHistory {
    let mut seed = symbol.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut closes = Vec::with_capacity(365);
    let mut p = rate * 0.92;
    for d in (0..365).rev() {
        let day = today - d;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let step = (f64::from(seed % 1000) / 1000.0 - 0.492) * 0.014;
        p = (p * (1.0 + step)).max(rate * 0.5);
        closes.push((day, p));
    }
    let last_gen = closes.last().map_or(rate, |c| c.1);
    let k = if last_gen > 0.0 { rate / last_gen } else { 1.0 };
    let n = closes.len().max(1) as f64;
    for (i, (_, c)) in closes.iter_mut().enumerate() {
        *c *= 1.0 + (k - 1.0) * (i as f64 / n);
    }
    if let Some(last) = closes.last_mut() {
        last.1 = rate;
    }
    ForexHistory { symbol: symbol.to_string(), closes }
}

pub fn today_num() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i32)
        .unwrap_or(20757)
}

pub fn fmt_idr(val: f64, is_gold: bool) -> String {
    let out = fmt_idr_plain(val);
    if is_gold {
        format!("Rp {} /g", out)
    } else {
        format!("Rp {}", out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_forex_and_gold() {
        let mut rates = HashMap::new();
        rates.insert("IDR".into(), 18132.0);
        rates.insert("EUR".into(), 0.8955);
        rates.insert("SGD".into(), 1.2782);
        rates.insert("JPY".into(), 157.49);
        rates.insert("SAR".into(), 3.75);

        let board = ForexBoard::from_rates(&rates, Some(3897.6), 1730366100);
        assert_eq!(board.quotes.len(), 12);

        let usd = board.quote("USD/IDR").unwrap();
        assert_eq!(usd.formatted_rate(), "Rp 18.132");
        assert_eq!(usd.spark.len(), 48);

        let gold = board.quote("GOLD").unwrap();
        assert!(gold.rate > 2_000_000.0);
        assert!(gold.formatted_rate().contains("/g"));
        // Backward-compatible alias lookup
        assert!(board.quote("EMAS").is_some());

        let silver = board.quote("SILVER").unwrap();
        assert!(silver.rate > 15_000.0);

        let oil = board.quote("OIL").unwrap();
        assert!(oil.rate > 50.0);

        let sgd = board.quote("SGD/IDR").unwrap();
        assert!(sgd.rate > 14_000.0);
    }
}
