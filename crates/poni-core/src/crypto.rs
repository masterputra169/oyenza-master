//! Cryptocurrency: Real-time crypto prices, charts and 24h statistics.
//!
//! Provides real-time quotes, 24h sparklines, historical charts (1D, 1M, 6M, 1Y),
//! and interactive watchlist for major cryptocurrencies (BTC, ETH, SOL, BNB, XRP, etc.).

use crate::datetime::days_from_civil;
use serde::{Deserialize, Serialize};

pub const MAX: usize = 50;
pub const STARTERS: [&str; 6] = ["BTC", "ETH", "SOL", "BNB", "XRP", "DOGE"];

const MINUS: char = '\u{2212}';

/// Top 120 CoinMarketCap cryptocurrencies actively listed on Binance (`<SYMBOL>USDT`),
/// ordered by CoinMarketCap market capitalization rank.
pub const KNOWN_COINS: &[(&str, &str)] = &[
    ("BTC", "Bitcoin"),
    ("ETH", "Ethereum"),
    ("XRP", "XRP"),
    ("BNB", "BNB"),
    ("SOL", "Solana"),
    ("DOGE", "Dogecoin"),
    ("ADA", "Cardano"),
    ("TRX", "TRON"),
    ("HYPE", "Hyperliquid"),
    ("TON", "Toncoin"),
    ("LINK", "Chainlink"),
    ("AVAX", "Avalanche"),
    ("SUI", "Sui"),
    ("XLM", "Stellar"),
    ("WBTC", "Wrapped Bitcoin"),
    ("SHIB", "Shiba Inu"),
    ("HBAR", "Hedera"),
    ("BCH", "Bitcoin Cash"),
    ("DOT", "Polkadot"),
    ("LTC", "Litecoin"),
    ("UNI", "Uniswap"),
    ("NEAR", "NEAR Protocol"),
    ("PEPE", "Pepe"),
    ("APT", "Aptos"),
    ("ICP", "Internet Computer"),
    ("AAVE", "Aave"),
    ("ETC", "Ethereum Classic"),
    ("ONDO", "Ondo"),
    ("TAO", "Bittensor"),
    ("RENDER", "Render"),
    ("POL", "Polygon"),
    ("VET", "VeChain"),
    ("FIL", "Filecoin"),
    ("ARB", "Arbitrum"),
    ("FET", "Artificial Superintelligence"),
    ("TRUMP", "OFFICIAL TRUMP"),
    ("OM", "MANTRA"),
    ("ALGO", "Algorand"),
    ("ATOM", "Cosmos"),
    ("OP", "Optimism"),
    ("ENA", "Ethena"),
    ("STX", "Stacks"),
    ("INJ", "Injective"),
    ("TIA", "Celestia"),
    ("IMX", "Immutable"),
    ("GRT", "The Graph"),
    ("BONK", "Bonk"),
    ("WLD", "Worldcoin"),
    ("S", "Sonic"),
    ("SEI", "Sei"),
    ("THETA", "Theta Network"),
    ("WIF", "dogwifhat"),
    ("FLOKI", "FLOKI"),
    ("JASMY", "JasmyCoin"),
    ("LDO", "Lido DAO"),
    ("SAND", "The Sandbox"),
    ("GALA", "Gala"),
    ("QNT", "Quant"),
    ("JUP", "Jupiter"),
    ("PYTH", "Pyth Network"),
    ("KAIA", "Kaia"),
    ("FLOW", "Flow"),
    ("XTZ", "Tezos"),
    ("IOTA", "IOTA"),
    ("BEAMX", "Beam"),
    ("NEO", "Neo"),
    ("AR", "Arweave"),
    ("CRV", "Curve DAO"),
    ("AXS", "Axie Infinity"),
    ("MANA", "Decentraland"),
    ("EGLD", "MultiversX"),
    ("PENDLE", "Pendle"),
    ("STRK", "Starknet"),
    ("DYDX", "dYdX"),
    ("RAY", "Raydium"),
    ("CAKE", "PancakeSwap"),
    ("ENS", "Ethereum Name Service"),
    ("ZEC", "Zcash"),
    ("CHZ", "Chiliz"),
    ("PAXG", "PAX Gold"),
    ("CFX", "Conflux"),
    ("APE", "ApeCoin"),
    ("RUNE", "THORChain"),
    ("MINA", "Mina"),
    ("SNX", "Synthetix"),
    ("COMP", "Compound"),
    ("ZK", "ZKsync"),
    ("EIGEN", "EigenLayer"),
    ("PENGU", "Pudgy Penguins"),
    ("VIRTUAL", "Virtuals Protocol"),
    ("BERA", "Berachain"),
    ("MOVE", "Movement"),
    ("NEXO", "Nexo"),
    ("DASH", "Dash"),
    ("KAVA", "Kava"),
    ("1INCH", "1inch Network"),
    ("W", "Wormhole"),
    ("CKB", "Nervos Network"),
    ("ROSE", "Oasis"),
    ("BLUR", "Blur"),
    ("JTO", "Jito"),
    ("ZRO", "LayerZero"),
    ("ORDI", "ORDI"),
    ("NOT", "Notcoin"),
    ("WOO", "WOO"),
    ("ASTR", "Astar"),
    ("GMT", "STEPN"),
    ("ZIL", "Zilliqa"),
    ("CELO", "Celo"),
    ("ANKR", "Ankr"),
    ("IOTX", "IoTeX"),
    ("QTUM", "Qtum"),
    ("KSM", "Kusama"),
    ("RSR", "Reserve Rights"),
    ("BAT", "Basic Attention Token"),
    ("ZRX", "0x Protocol"),
    ("ENJ", "Enjin Coin"),
    ("LRC", "Loopring"),
    ("HOT", "Holo"),
    ("RVN", "Ravencoin"),
    ("SUSHI", "SushiSwap"),
    ("YFI", "yearn.finance"),
    ("MKR", "Maker"),
    ("SKY", "Sky"),
    ("AERO", "Aerodrome Finance"),
    ("MORPHO", "Morpho"),
    ("ETHFI", "ether.fi"),
    ("GNO", "Gnosis"),
    ("KAITO", "KAITO"),
    ("SYRUP", "Maple Finance"),
    ("WLFI", "World Liberty Financial"),
    ("LINEA", "Linea"),
    ("PUMP", "Pump.fun"),
    ("ASTER", "Aster"),
    ("PLUME", "Plume"),
    ("KMNO", "Kamino Finance"),
    ("ORCA", "Orca"),
    ("PNUT", "Peanut the Squirrel"),
];

pub fn coin_name(code: &str) -> &'static str {
    for &(c, n) in KNOWN_COINS {
        if c.eq_ignore_ascii_case(code) {
            return n;
        }
    }
    ""
}

pub fn is_code(s: &str) -> bool {
    let s = s.trim();
    (1..=10).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn code_of(s: &str) -> Option<String> {
    let c = s.trim().to_ascii_uppercase();
    is_code(&c).then_some(c)
}

/// Searches `KNOWN_COINS`, excluding coins already in `skip`.
/// When `query` is empty, returns all remaining coins in CoinMarketCap rank order.
/// When `query` is non-empty, ranks exact symbol matches first, then symbol prefix,
/// name prefix, word prefix, and substring matches.
pub fn search_coins(query: &str, skip: &[String]) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_ascii_uppercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, usize, &'static str, &'static str)> = Vec::new();
    for (idx, &(c, n)) in KNOWN_COINS.iter().enumerate() {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Show {
    #[default]
    Percent,
    Points,
    Cap,
}

impl Show {
    pub fn next(self) -> Show {
        match self {
            Show::Percent => Show::Points,
            Show::Points => Show::Cap,
            Show::Cap => Show::Percent,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range {
    #[default]
    #[serde(rename = "1d")]
    Day,
    #[serde(rename = "1m")]
    Month,
    #[serde(rename = "6m")]
    HalfYear,
    #[serde(rename = "1y")]
    Year,
}

impl Range {
    pub const ALL: [Range; 4] = [Range::Day, Range::Month, Range::HalfYear, Range::Year];

    pub fn label(self) -> &'static str {
        match self {
            Range::Day => "1D",
            Range::Month => "1M",
            Range::HalfYear => "6M",
            Range::Year => "1Y",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trend {
    Up,
    Down,
    Flat,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CryptoQuote {
    pub code: String,
    pub name: String,
    pub price: f64,
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub market_cap: f64,
    pub spark: Vec<(u16, f32)>,
}

impl CryptoQuote {
    pub fn change(&self) -> f64 {
        self.price - self.prev_close
    }

    pub fn change_pct(&self) -> f64 {
        if self.prev_close > 0.0 {
            (self.price - self.prev_close) / self.prev_close * 100.0
        } else {
            0.0
        }
    }

    pub fn trend(&self) -> Trend {
        let diff = self.change();
        if diff.abs() < 1e-6 {
            Trend::Flat
        } else if diff > 0.0 {
            Trend::Up
        } else {
            Trend::Down
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CryptoBoard {
    pub at: i64,
    pub quotes: Vec<CryptoQuote>,
    pub missing: Vec<String>,
}

impl CryptoBoard {
    pub fn quote(&self, code: &str) -> Option<&CryptoQuote> {
        self.quotes.iter().find(|q| q.code.eq_ignore_ascii_case(code))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CryptoWatchlist {
    pub codes: Vec<String>,
    pub pick: String,
    #[serde(default)]
    pub show: Show,
    #[serde(default)]
    pub range: Range,
}

impl Default for CryptoWatchlist {
    fn default() -> Self {
        Self {
            codes: STARTERS.iter().map(|&s| s.to_string()).collect(),
            pick: STARTERS[0].to_string(),
            show: Show::Percent,
            range: Range::Day,
        }
    }
}

impl CryptoWatchlist {
    pub fn full(&self) -> bool {
        self.codes.len() >= MAX
    }

    pub fn add(&mut self, code: &str) -> bool {
        if self.full() || self.codes.iter().any(|c| c.eq_ignore_ascii_case(code)) {
            return false;
        }
        self.codes.push(code.to_ascii_uppercase());
        true
    }

    pub fn remove(&mut self, code: &str) {
        if let Some(pos) = self.codes.iter().position(|c| c.eq_ignore_ascii_case(code)) {
            self.codes.remove(pos);
            if self.pick.eq_ignore_ascii_case(code) {
                self.pick = self.codes.first().cloned().unwrap_or_else(|| "BTC".to_string());
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CryptoHistory {
    pub code: String,
    pub closes: Vec<(i32, f64)>,
}

pub fn fmt_price(price: f64) -> String {
    if price >= 1000.0 {
        let whole = price.round() as i64;
        let mut s = whole.to_string();
        let mut i = s.len() as isize - 3;
        while i > 0 {
            s.insert(i as usize, ',');
            i -= 3;
        }
        format!("${s}")
    } else if price >= 1.0 {
        format!("${price:.2}")
    } else if price >= 0.01 {
        format!("${price:.4}")
    } else if price >= 0.0001 {
        format!("${price:.6}")
    } else if price > 0.0 {
        format!("${price:.8}")
    } else {
        "$0.00".to_string()
    }
}

pub fn fmt_change(q: &CryptoQuote, show: Show) -> String {
    match show {
        Show::Percent => {
            let pct = q.change_pct();
            if pct < 0.0 {
                format!("{MINUS}{:.2}%", pct.abs())
            } else {
                format!("+{:.2}%", pct)
            }
        }
        Show::Points => {
            let chg = q.change();
            if chg < 0.0 {
                format!("{MINUS}{}", fmt_price(chg.abs()))
            } else {
                format!("+{}", fmt_price(chg))
            }
        }
        Show::Cap => fmt_cap(q.market_cap),
    }
}

pub fn fmt_volume(vol: f64) -> String {
    if vol >= 1e9 {
        format!("${:.2}B", vol / 1e9)
    } else if vol >= 1e6 {
        format!("${:.1}M", vol / 1e6)
    } else if vol >= 1e3 {
        format!("${:.1}K", vol / 1e3)
    } else {
        format!("${:.0}", vol)
    }
}

pub fn fmt_cap(cap: f64) -> String {
    if cap >= 1e12 {
        format!("${:.2}T", cap / 1e12)
    } else if cap >= 1e9 {
        format!("${:.1}B", cap / 1e9)
    } else if cap >= 1e6 {
        format!("${:.1}M", cap / 1e6)
    } else {
        format!("${:.0}", cap)
    }
}

pub fn bounds(values: impl IntoIterator<Item = f64>, base: Option<f64>) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in values.into_iter().chain(base).filter(|v| v.is_finite()) {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if lo > hi {
        return None;
    }
    let pad = ((hi - lo) * 0.08).max(hi.abs() * 0.0015).max(1e-6);
    Some((lo - pad, hi + pad))
}

pub fn resample(pts: &[(f32, f32)], count: usize) -> Vec<(f32, f32)> {
    if pts.is_empty() {
        return vec![(0.0, 0.5); count];
    }
    if pts.len() == 1 {
        return vec![pts[0]; count];
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / (count - 1) as f32;
        let mut found = false;
        for j in 0..pts.len() - 1 {
            let (x0, y0) = pts[j];
            let (x1, y1) = pts[j + 1];
            if t >= x0 && t <= x1 {
                let u = if (x1 - x0).abs() < 1e-5 { 0.0 } else { (t - x0) / (x1 - x0) };
                out.push((t, y0 + (y1 - y0) * u));
                found = true;
                break;
            }
        }
        if !found {
            if t <= pts[0].0 {
                out.push((t, pts[0].1));
            } else {
                out.push((t, pts.last().unwrap().1));
            }
        }
    }
    out
}

/// Generates a realistic 48-point (30m interval) 24h intraday curve between `prev` and `price`.
pub fn synthetic_spark(code: &str, prev: f64, price: f64, low: f64, high: f64) -> Vec<(u16, f32)> {
    let mut seed = code.bytes().fold(0x9E37_79B9u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut spark = Vec::with_capacity(48);
    let swing = (high - low).max((price - prev).abs()).max(prev * 0.008);
    let mut lean = 0.0;
    let steps = 47u16;
    for k in 0..=steps {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        lean = lean * 0.80 + (f64::from(seed % 1000) / 1000.0 - 0.5) * swing * 0.38;
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

pub fn sample_board() -> CryptoBoard {
    let now = 1793374000;
    let items = [
        ("BTC", "Bitcoin", 77630.0, 75200.0, 78200.0, 74900.0, 34000.0, 2.7e9, 1.53e12),
        ("ETH", "Ethereum", 2240.50, 2200.0, 2265.0, 2170.0, 620000.0, 1.4e9, 269.0e9),
        ("SOL", "Solana", 94.95, 90.80, 97.40, 89.50, 4800000.0, 470.0e6, 44.8e9),
        ("BNB", "BNB", 642.10, 636.0, 649.0, 631.0, 350000.0, 224.0e6, 93.5e9),
        ("XRP", "XRP", 1.85, 1.88, 1.92, 1.81, 150000000.0, 277.0e6, 105.0e9),
        ("DOGE", "Dogecoin", 0.165, 0.160, 0.171, 0.158, 800000000.0, 132.0e6, 24.2e9),
        ("HYPE", "Hyperliquid", 88.82, 93.31, 94.02, 88.35, 467862.0, 42.8e6, 21.25e9),
        ("ADA", "Cardano", 0.725, 0.710, 0.740, 0.702, 120000000.0, 87.0e6, 25.8e9),
        ("SUI", "Sui", 3.15, 2.96, 3.25, 2.91, 75000000.0, 236.0e6, 8.9e9),
    ];
    let mut quotes = Vec::new();
    for &(code, name, price, prev, high, low, vol, qvol, cap) in &items {
        let spark = synthetic_spark(code, prev, price, low, high);
        quotes.push(CryptoQuote {
            code: code.to_string(),
            name: name.to_string(),
            price,
            prev_close: prev,
            open: prev,
            high,
            low,
            volume: vol,
            quote_volume: qvol,
            market_cap: cap,
            spark,
        });
    }
    CryptoBoard { at: now, quotes, missing: Vec::new() }
}

pub fn sample_history(code: &str, today: i32, price: f64) -> CryptoHistory {
    let mut seed = code.bytes().fold(0x2545_F491u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193));
    let mut closes = Vec::with_capacity(365);
    let mut p = price * 0.72;
    for d in (0..365).rev() {
        let day = today - d;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let step = (f64::from(seed % 1000) / 1000.0 - 0.485) * 0.045;
        p = (p * (1.0 + step)).max(price * 0.15);
        closes.push((day, p));
    }
    let last_gen = closes.last().map_or(price, |c| c.1);
    let k = if last_gen > 0.0 { price / last_gen } else { 1.0 };
    let n = closes.len().max(1) as f64;
    for (i, (_, c)) in closes.iter_mut().enumerate() {
        *c *= 1.0 + (k - 1.0) * (i as f64 / n);
    }
    if let Some(last) = closes.last_mut() {
        last.1 = price;
    }
    CryptoHistory { code: code.to_string(), closes }
}

pub fn today_num() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i32)
        .unwrap_or_else(|_| days_from_civil(2026, 10, 31) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn known_coins_has_top_100_unique_codes() {
        assert!(KNOWN_COINS.len() >= 100);
        let mut seen = HashSet::new();
        for &(code, name) in KNOWN_COINS {
            assert!(is_code(code), "invalid code {code}");
            assert!(!name.is_empty(), "empty name for {code}");
            assert!(seen.insert(code), "duplicate code {code}");
        }
    }

    #[test]
    fn search_coins_ranks_exact_and_prefix() {
        let skip = vec!["BTC".to_string()];
        let all = search_coins("", &skip);
        assert!(all.is_empty());

        let s_hits = search_coins("S", &[]);
        assert_eq!(s_hits.first().map(|p| p.0), Some("S"));

        let sol_hits = search_coins("sol", &[]);
        assert_eq!(sol_hits.first().map(|p| p.0), Some("SOL"));
    }
}
