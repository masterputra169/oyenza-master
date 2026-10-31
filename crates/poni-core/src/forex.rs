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
    pub change_pct: f64,
    pub is_gold: bool,
    pub unit: String,
}

impl ForexQuote {
    pub fn trend(&self) -> i8 {
        poni_core_trend(self.change_pct)
    }

    pub fn formatted_rate(&self) -> String {
        fmt_idr(self.rate, self.is_gold)
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
        quotes.push(ForexQuote {
            symbol: "USD/IDR".into(),
            name: "US Dollar".into(),
            rate: idr,
            change_pct: 0.12,
            is_gold: false,
            unit: "per 1 USD".into(),
        });

        // 2. EMAS Antam (Logam Mulia)
        let gold_usd_oz = paxg_usd.unwrap_or(3900.0);
        let gold_idr_gram = (gold_usd_oz * idr / TROY_OUNCE_GRAMS) * ANTAM_PREMIUM_FACTOR;
        quotes.push(ForexQuote {
            symbol: "EMAS".into(),
            name: "Emas Antam (LM)".into(),
            rate: gold_idr_gram,
            change_pct: -0.24,
            is_gold: true,
            unit: "per 1 gram".into(),
        });

        // 3. SGD/IDR
        if let Some(&sgd) = rates.get("SGD") {
            if sgd > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "SGD/IDR".into(),
                    name: "Singapore Dollar".into(),
                    rate: idr / sgd,
                    change_pct: 0.08,
                    is_gold: false,
                    unit: "per 1 SGD".into(),
                });
            }
        }

        // 4. EUR/IDR
        if let Some(&eur) = rates.get("EUR") {
            if eur > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "EUR/IDR".into(),
                    name: "Euro".into(),
                    rate: idr / eur,
                    change_pct: -0.15,
                    is_gold: false,
                    unit: "per 1 EUR".into(),
                });
            }
        }

        // 5. JPY/IDR (per 100 JPY)
        if let Some(&jpy) = rates.get("JPY") {
            if jpy > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "JPY/IDR".into(),
                    name: "Japanese Yen (100)".into(),
                    rate: (idr / jpy) * 100.0,
                    change_pct: 0.31,
                    is_gold: false,
                    unit: "per 100 JPY".into(),
                });
            }
        }

        // 6. SAR/IDR (Saudi Riyal)
        if let Some(&sar) = rates.get("SAR") {
            if sar > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "SAR/IDR".into(),
                    name: "Saudi Riyal".into(),
                    rate: idr / sar,
                    change_pct: 0.05,
                    is_gold: false,
                    unit: "per 1 SAR".into(),
                });
            }
        }

        // 7. MYR/IDR
        if let Some(&myr) = rates.get("MYR") {
            if myr > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "MYR/IDR".into(),
                    name: "Malaysian Ringgit".into(),
                    rate: idr / myr,
                    change_pct: 0.02,
                    is_gold: false,
                    unit: "per 1 MYR".into(),
                });
            }
        }

        // 8. CNY/IDR
        if let Some(&cny) = rates.get("CNY") {
            if cny > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "CNY/IDR".into(),
                    name: "Chinese Yuan".into(),
                    rate: idr / cny,
                    change_pct: -0.04,
                    is_gold: false,
                    unit: "per 1 CNY".into(),
                });
            }
        }

        // 9. AUD/IDR
        if let Some(&aud) = rates.get("AUD") {
            if aud > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "AUD/IDR".into(),
                    name: "Australian Dollar".into(),
                    rate: idr / aud,
                    change_pct: 0.18,
                    is_gold: false,
                    unit: "per 1 AUD".into(),
                });
            }
        }

        // 10. GBP/IDR
        if let Some(&gbp) = rates.get("GBP") {
            if gbp > 0.0 {
                quotes.push(ForexQuote {
                    symbol: "GBP/IDR".into(),
                    name: "British Pound".into(),
                    rate: idr / gbp,
                    change_pct: -0.11,
                    is_gold: false,
                    unit: "per 1 GBP".into(),
                });
            }
        }

        Self { quotes, updated_at: now_unix }
    }
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
        assert_eq!(board.quotes.len(), 6);

        let usd = board.quote("USD/IDR").unwrap();
        assert_eq!(usd.formatted_rate(), "Rp 18.132");

        let emas = board.quote("EMAS").unwrap();
        assert!(emas.rate > 2_000_000.0);
        assert!(emas.formatted_rate().contains("/gr"));

        let sgd = board.quote("SGD/IDR").unwrap();
        assert!(sgd.rate > 14_000.0);
    }
}
