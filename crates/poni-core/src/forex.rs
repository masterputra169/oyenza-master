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

/// Maximum number of forex & commodities a user can hold in their watchlist.
pub const MAX_FOREX: usize = 20;

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

/// Top currencies and commodities available for search & addition.
pub const KNOWN_FOREX_ITEMS: [(&str, &str); 28] = [
    ("USD/IDR", "US Dollar"),
    ("GOLD", "Gold (Antam / g)"),
    ("SILVER", "Silver (Spot / g)"),
    ("OIL", "WTI Crude Oil (USD)"),
    ("SGD/IDR", "Singapore Dollar"),
    ("EUR/IDR", "Euro"),
    ("JPY/IDR", "Japanese Yen (100)"),
    ("SAR/IDR", "Saudi Riyal"),
    ("MYR/IDR", "Malaysian Ringgit"),
    ("CNY/IDR", "Chinese Yuan"),
    ("AUD/IDR", "Australian Dollar"),
    ("GBP/IDR", "British Pound"),
    ("CHF/IDR", "Swiss Franc"),
    ("CAD/IDR", "Canadian Dollar"),
    ("HKD/IDR", "Hong Kong Dollar"),
    ("KRW/IDR", "South Korean Won (100)"),
    ("THB/IDR", "Thai Baht"),
    ("NZD/IDR", "New Zealand Dollar"),
    ("TWD/IDR", "New Taiwan Dollar"),
    ("INR/IDR", "Indian Rupee"),
    ("AED/IDR", "UAE Dirham"),
    ("TRY/IDR", "Turkish Lira"),
    ("PHP/IDR", "Philippine Peso"),
    ("PLATINUM", "Platinum Spot (per g)"),
    ("BRENT", "Brent Crude Oil (USD)"),
    ("NATURAL_GAS", "Natural Gas (USD)"),
    ("COPPER", "Copper Spot (USD/lb)"),
    ("PALLADIUM", "Palladium Spot (per g)"),
];

/// The user's persisted Forex & Commodities watchlist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForexWatchlist {
    pub symbols: Vec<String>,
    pub pick: String,
    #[serde(default)]
    pub show_pct: bool,
    #[serde(default)]
    pub range: crate::stocks::Range,
}

impl Default for ForexWatchlist {
    fn default() -> Self {
        Self {
            symbols: DEFAULT_SYMBOLS.iter().map(|&s| s.to_string()).collect(),
            pick: "USD/IDR".to_string(),
            show_pct: true,
            range: crate::stocks::Range::Day,
        }
    }
}

impl ForexWatchlist {
    pub fn full(&self) -> bool {
        self.symbols.len() >= MAX_FOREX
    }

    pub fn has(&self, sym: &str) -> bool {
        let norm = normalize_symbol(sym);
        self.symbols.iter().any(|s| normalize_symbol(s).eq_ignore_ascii_case(&norm))
    }

    pub fn add(&mut self, sym: &str) -> bool {
        let norm = normalize_symbol(sym);
        if self.full() || self.has(&norm) || norm.is_empty() {
            return false;
        }
        self.symbols.push(norm);
        true
    }

    pub fn remove(&mut self, sym: &str) {
        let norm = normalize_symbol(sym);
        if let Some(pos) = self.symbols.iter().position(|s| normalize_symbol(s).eq_ignore_ascii_case(&norm)) {
            self.symbols.remove(pos);
            if normalize_symbol(&self.pick).eq_ignore_ascii_case(&norm) {
                self.pick = self.symbols.first().cloned().unwrap_or_else(|| "USD/IDR".to_string());
            }
        }
    }

    pub fn move_to(&mut self, sym: &str, target_idx: usize) {
        let norm = normalize_symbol(sym);
        if let Some(cur) = self.symbols.iter().position(|s| normalize_symbol(s).eq_ignore_ascii_case(&norm)) {
            let item = self.symbols.remove(cur);
            let to = target_idx.min(self.symbols.len());
            self.symbols.insert(to, item);
        }
    }
}

/// Autocomplete and search helper over known currencies and commodities.
pub fn search_forex(query: &str, skip: &[String]) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_ascii_uppercase();
    let norm_skip: Vec<String> = skip.iter().map(|s| normalize_symbol(s)).collect();
    if q.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, usize, &'static str, &'static str)> = Vec::new();
    for (idx, &(sym, name)) in KNOWN_FOREX_ITEMS.iter().enumerate() {
        let norm_sym = normalize_symbol(sym);
        if norm_skip.iter().any(|x| x.eq_ignore_ascii_case(&norm_sym)) {
            continue;
        }
        let su = sym.to_ascii_uppercase();
        let nu = name.to_ascii_uppercase();
        let rank = if su == q || norm_sym == q {
            0
        } else if su.starts_with(&q) || norm_sym.starts_with(&q) {
            1
        } else if nu == q {
            2
        } else if nu.starts_with(&q) {
            3
        } else if nu.split_whitespace().any(|w| w.starts_with(&q)) {
            4
        } else if su.contains(&q) || norm_sym.contains(&q) {
            5
        } else if nu.contains(&q) {
            6
        } else {
            continue;
        };
        ranked.push((rank, idx, sym, name));
    }
    ranked.sort_by_key(|&(rank, idx, _, _)| (rank, idx));
    ranked.into_iter().map(|(_, _, s, n)| (s, n)).collect()
}

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
        matches!(self.symbol.as_str(), "OIL" | "MINYAK" | "WTI" | "BRENT" | "COPPER" | "NATURAL_GAS" | "TEMBAGA" | "GAS")
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
        "OIL" | "MINYAK" | "WTI" | "BRENT" | "COPPER" | "NATURAL_GAS" | "TEMBAGA" | "GAS" => format!("${:.2}", val),
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
        rates.insert("IDR".into(), 16250.0);
        rates.insert("SGD".into(), 1.2782);
        rates.insert("EUR".into(), 0.8956);
        rates.insert("JPY".into(), 157.49);
        rates.insert("SAR".into(), 3.7500);
        rates.insert("MYR".into(), 4.3850);
        rates.insert("CNY".into(), 7.1250);
        rates.insert("AUD".into(), 1.5180);
        rates.insert("GBP".into(), 0.7715);
        Self::from_rates(&rates, Some(2735.0), 0)
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
        let syms: Vec<String> = DEFAULT_SYMBOLS.iter().map(|&s| s.to_string()).collect();
        Self::from_rates_feeds_and_symbols(rates, feeds, &syms, now_unix)
    }

    pub fn from_rates_feeds_and_symbols(rates: &HashMap<String, f64>, feeds: &CommodityFeeds, symbols: &[String], now_unix: u64) -> Self {
        let idr = *rates.get("IDR").unwrap_or(&18132.0);
        let mut quotes = Vec::with_capacity(symbols.len());

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

        for sym_raw in symbols {
            let sym = normalize_symbol(sym_raw);
            match sym.as_str() {
                "USD/IDR" => {
                    quotes.push(build("USD/IDR", "US Dollar", idr, 0.12, false, "per 1 USD"));
                }
                "GOLD" => {
                    let (gold_usd_oz, gold_pct) = feeds.gold_usd.unwrap_or((2735.0, -0.24));
                    let gold_idr_gram = (gold_usd_oz * idr / TROY_OUNCE_GRAMS) * ANTAM_PREMIUM_FACTOR;
                    quotes.push(build("GOLD", "Gold (Antam / g)", gold_idr_gram, gold_pct, true, "per 1 gram"));
                }
                "SILVER" => {
                    let (silver_usd_oz, silver_pct) = feeds.silver_usd.unwrap_or((32.50, 0.45));
                    let silver_idr_gram = (silver_usd_oz * idr / TROY_OUNCE_GRAMS) * 1.05;
                    quotes.push(build("SILVER", "Silver (Spot / g)", silver_idr_gram, silver_pct, false, "per 1 gram"));
                }
                "OIL" => {
                    let (oil_usd_barrel, oil_pct) = feeds.oil_usd.unwrap_or((74.50, -0.62));
                    quotes.push(build("OIL", "WTI Crude Oil", oil_usd_barrel, oil_pct, false, "per barrel (USD)"));
                }
                "BRENT" => {
                    let brent_usd = 78.40;
                    quotes.push(build("BRENT", "Brent Crude Oil", brent_usd, -0.45, false, "per barrel (USD)"));
                }
                "PLATINUM" => {
                    let plat_usd_g = 32.50;
                    quotes.push(build("PLATINUM", "Platinum Spot / g", plat_usd_g * idr, 0.20, false, "per 1 gram"));
                }
                "PALLADIUM" => {
                    let pal_usd_g = 35.80;
                    quotes.push(build("PALLADIUM", "Palladium Spot / g", pal_usd_g * idr, -0.30, false, "per 1 gram"));
                }
                "NATURAL_GAS" => {
                    quotes.push(build("NATURAL_GAS", "Natural Gas", 2.85, 1.10, false, "per MMBtu (USD)"));
                }
                "COPPER" => {
                    quotes.push(build("COPPER", "Copper Spot", 4.38, 0.35, false, "per lb (USD)"));
                }
                other => {
                    let cur_code = other.split('/').next().unwrap_or(other).to_ascii_uppercase();
                    let cur_rate = rates.get(&cur_code).copied().filter(|&v| v > 0.0);
                    let fallback_rate = match cur_code.as_str() {
                        "SGD" => Some(1.2782),
                        "EUR" => Some(0.8956),
                        "JPY" => Some(157.49),
                        "SAR" => Some(3.7500),
                        "MYR" => Some(4.3850),
                        "CNY" => Some(7.1250),
                        "AUD" => Some(1.5180),
                        "GBP" => Some(0.7715),
                        "CHF" => Some(0.8650),
                        "CAD" => Some(1.3850),
                        "HKD" => Some(7.7720),
                        "KRW" => Some(1375.0),
                        "THB" => Some(33.80),
                        "NZD" => Some(1.6850),
                        "TWD" => Some(32.10),
                        "INR" => Some(84.10),
                        "AED" => Some(3.6725),
                        "TRY" => Some(34.20),
                        "PHP" => Some(58.30),
                        _ => None,
                    };
                    let r = cur_rate.or(fallback_rate).unwrap_or(1.0);
                    let (val, unit) = if cur_code == "JPY" || cur_code == "KRW" {
                        ((idr / r) * 100.0, format!("per 100 {cur_code}"))
                    } else if cur_code == "VND" {
                        ((idr / r) * 1000.0, format!("per 1000 {cur_code}"))
                    } else {
                        (idr / r, format!("per 1 {cur_code}"))
                    };
                    let known_name = KNOWN_FOREX_ITEMS.iter().find(|(s, _)| s.starts_with(&cur_code)).map(|(_, n)| *n);
                    let display_name = known_name.unwrap_or(other);
                    quotes.push(build(&format!("{cur_code}/IDR"), display_name, val, 0.05, false, &unit));
                }
            }
        }

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
        rates.insert("IDR".into(), 16250.0);
        rates.insert("EUR".into(), 0.8955);
        rates.insert("SGD".into(), 1.2782);
        rates.insert("JPY".into(), 157.49);
        rates.insert("SAR".into(), 3.75);

        let board = ForexBoard::from_rates(&rates, Some(2735.0), 1730366100);
        assert_eq!(board.quotes.len(), 12);

        let usd = board.quote("USD/IDR").unwrap();
        assert_eq!(usd.formatted_rate(), "Rp 16.250");
        assert_eq!(usd.spark.len(), 48);

        let gold = board.quote("GOLD").unwrap();
        assert!(gold.rate > 1_400_000.0 && gold.rate < 1_600_000.0);
        assert!(gold.formatted_rate().contains("/g"));
        // Backward-compatible alias lookup
        assert!(board.quote("EMAS").is_some());

        let silver = board.quote("SILVER").unwrap();
        assert!(silver.rate > 15_000.0);

        let oil = board.quote("OIL").unwrap();
        assert!(oil.rate > 50.0);

        let sgd = board.quote("SGD/IDR").unwrap();
        assert!(sgd.rate > 12_000.0);
    }

    #[test]
    fn forex_watchlist_add_remove_search() {
        let mut wl = ForexWatchlist::default();
        assert!(wl.has("USD/IDR"));
        assert!(!wl.has("CHF/IDR"));
        assert!(wl.add("CHF/IDR"));
        assert!(wl.has("CHF/IDR"));
        wl.remove("CHF/IDR");
        assert!(!wl.has("CHF/IDR"));

        let hits = search_forex("GOLD", &[]);
        assert!(!hits.is_empty());
        assert_eq!(hits[0].0, "GOLD");
    }
}
