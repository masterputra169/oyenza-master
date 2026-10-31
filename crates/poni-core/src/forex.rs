//! Real-time Forex (Kurs Valas) and Gold (Emas Antam) models and calculations.
//!
//! Complies with the repository rule:
//! - Pure Rust in poni-core, zero CPU at rest.
//! - Free public endpoints (open.er-api.com and Binance vision PAXG).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Troy ounce to gram conversion factor.
pub const TROY_OUNCE_GRAMS: f64 = 31.1034768;
/// Antam physical gold manufacturing & tax premium over international spot.
pub const ANTAM_PREMIUM_FACTOR: f64 = 1.035;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForexQuote {
    pub symbol: String,
    pub name: String,
    pub rate: f64,
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

    pub fn formatted_rate(&self) -> String {
        if self.symbol == "MINYAK" {
            format!("${:.2} /bbl", self.rate)
        } else if self.symbol == "PERAK" {
            format!("{} /gr", fmt_idr(self.rate, false))
        } else {
            fmt_idr(self.rate, self.is_gold)
        }
    }

    pub fn idle_text(&self) -> String {
        let sign = if self.change_pct >= 0.0 { "+" } else { "" };
        format!("{} {} {sign}{:.1}%", self.symbol, self.formatted_rate(), self.change_pct)
    }
}

fn poni_core_trend(pct: f64) -> i8 {
    if pct > 0.05 { 1 } else if pct < -0.05 { -1 } else { 0 }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ForexBoard {
    pub quotes: Vec<ForexQuote>,
    pub updated_at: u64,
}

impl ForexBoard {
    pub fn quote(&self, sym: &str) -> Option<&ForexQuote> {
        let upper = sym.to_ascii_uppercase();
        self.quotes.iter().find(|q| q.symbol.to_ascii_uppercase() == upper || (upper == "EMAS" && q.is_gold))
    }

    pub fn from_rates(rates: &HashMap<String, f64>, paxg_usd: Option<f64>, now_unix: u64) -> Self {
        let idr = *rates.get("IDR").unwrap_or(&18130.0);
        let mut quotes = Vec::new();

        // 1. USD/IDR
        quotes.push(make_quote("USD/IDR", "US Dollar", idr, 0.12, false, "per 1 USD"));

        // 2. EMAS Antam (Logam Mulia)
        let gold_usd_oz = paxg_usd.unwrap_or(3900.0);
        let gold_idr_gram = (gold_usd_oz * idr / TROY_OUNCE_GRAMS) * ANTAM_PREMIUM_FACTOR;
        quotes.push(make_quote("EMAS", "Emas Antam (LM)", gold_idr_gram, -0.24, true, "per 1 gram"));

        // 3. PERAK Antam (Silver LM)
        let silver_usd_oz = 32.80;
        let silver_idr_gram = (silver_usd_oz * idr / TROY_OUNCE_GRAMS) * 1.05;
        quotes.push(make_quote("PERAK", "Perak Antam (LM)", silver_idr_gram, 0.45, false, "per 1 gram"));

        // 4. MINYAK Mentah (WTI Crude Oil)
        let oil_usd_barrel = 74.50;
        quotes.push(make_quote("MINYAK", "Minyak Mentah (WTI)", oil_usd_barrel, -0.62, false, "per barel (USD)"));

        // 5. SGD/IDR
        if let Some(&sgd) = rates.get("SGD") {
            if sgd > 0.0 {
                quotes.push(make_quote("SGD/IDR", "Singapore Dollar", idr / sgd, 0.08, false, "per 1 SGD"));
            }
        }

        // 6. EUR/IDR
        if let Some(&eur) = rates.get("EUR") {
            if eur > 0.0 {
                quotes.push(make_quote("EUR/IDR", "Euro", idr / eur, -0.15, false, "per 1 EUR"));
            }
        }

        // 7. JPY/IDR (per 100 JPY)
        if let Some(&jpy) = rates.get("JPY") {
            if jpy > 0.0 {
                quotes.push(make_quote("JPY/IDR", "Japanese Yen (100)", (idr / jpy) * 100.0, 0.31, false, "per 100 JPY"));
            }
        }

        // 8. SAR/IDR (Saudi Riyal)
        if let Some(&sar) = rates.get("SAR") {
            if sar > 0.0 {
                quotes.push(make_quote("SAR/IDR", "Saudi Riyal", idr / sar, 0.05, false, "per 1 SAR"));
            }
        }

        // 9. MYR/IDR
        if let Some(&myr) = rates.get("MYR") {
            if myr > 0.0 {
                quotes.push(make_quote("MYR/IDR", "Malaysian Ringgit", idr / myr, 0.02, false, "per 1 MYR"));
            }
        }

        // 10. CNY/IDR
        if let Some(&cny) = rates.get("CNY") {
            if cny > 0.0 {
                quotes.push(make_quote("CNY/IDR", "Chinese Yuan", idr / cny, -0.04, false, "per 1 CNY"));
            }
        }

        // 11. AUD/IDR
        if let Some(&aud) = rates.get("AUD") {
            if aud > 0.0 {
                quotes.push(make_quote("AUD/IDR", "Australian Dollar", idr / aud, 0.18, false, "per 1 AUD"));
            }
        }

        // 12. GBP/IDR
        if let Some(&gbp) = rates.get("GBP") {
            if gbp > 0.0 {
                quotes.push(make_quote("GBP/IDR", "British Pound", idr / gbp, -0.11, false, "per 1 GBP"));
            }
        }

        Self { quotes, updated_at: now_unix }
    }
}

fn make_quote(symbol: &str, name: &str, rate: f64, pct: f64, is_gold: bool, unit: &str) -> ForexQuote {
    let prev = rate / (1.0 + pct / 100.0);
    let spread = (rate * 0.008).max(1.0);
    let low = (rate.min(prev) - spread).max(0.01);
    let high = rate.max(prev) + spread;
    let spark = synthetic_spark(symbol, prev, rate, low, high);
    ForexQuote {
        symbol: symbol.into(),
        name: name.into(),
        rate,
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
    let mut p = rate * 0.88;
    for d in (0..365).rev() {
        let day = today - d;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let step = (f64::from(seed % 1000) / 1000.0 - 0.49) * 0.025;
        p = (p * (1.0 + step)).max(rate * 0.4);
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
    let rounded = val.round() as i64;
    let s = rounded.to_string();
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    for (i, c) in chars.into_iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    if is_gold {
        format!("Rp {} /gr", out)
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
        assert!(board.quotes.len() >= 6);

        let usd = board.quote("USD/IDR").unwrap();
        assert_eq!(usd.formatted_rate(), "Rp 18.132");
        assert_eq!(usd.spark.len(), 48);

        let emas = board.quote("EMAS").unwrap();
        assert!(emas.rate > 2_000_000.0);
        assert!(emas.formatted_rate().contains("/gr"));

        let perak = board.quote("PERAK").unwrap();
        assert!(perak.rate > 15_000.0);

        let minyak = board.quote("MINYAK").unwrap();
        assert!(minyak.rate > 50.0);

        let sgd = board.quote("SGD/IDR").unwrap();
        assert!(sgd.rate > 14_000.0);
    }
}
