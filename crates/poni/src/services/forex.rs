//! Forex & Commodities service: fetches real-time exchange rates, Gold/Silver/Oil spot prices,
//! and intraday sparklines from open.er-api.com, jsdelivr currency-api, Binance PAXG, and Yahoo Finance.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   not watched (`watch.open == false`).
//! - Free public endpoints, zero API keys.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::forex::{CommodityFeeds, ForexBoard, ForexQuote};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) OyenzaStudio/0.2.24";
const TIMEOUT: Duration = Duration::from_secs(6);
const REFRESH_GAP: Duration = Duration::from_secs(120);
#[allow(dead_code)]
pub const DOLLARKILAT_ENDPOINT: &str = "https://api.dollarkilat.xyz";
pub const HUB_ENDPOINTS: &[&str] = &[
    "https://api.dollarkilat.xyz",
    "https://oyenza-master-api.pages.dev",
];
 // 2 minutes while open

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    pub open: bool,
    pub chart: Option<String>,
    pub symbols: Vec<String>,
    pub seq: u64,
}

pub struct ForexService {
    state: Arc<(Mutex<WorkerState>, Condvar)>,
}

#[derive(Default)]
struct WorkerState {
    watch: Watch,
    wake: bool,
    stop: bool,
}

#[derive(Deserialize)]
struct ExchangeApiResponse {
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct BinanceTickerResponse {
    #[serde(rename = "lastPrice")]
    last_price: String,
    #[serde(rename = "priceChangePercent", default)]
    price_change_pct: String,
}

impl ForexService {
    pub fn start(sink: EventSink) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(WorkerState::default()), Condvar::new()));
        let s = state.clone();
        std::thread::Builder::new()
            .name("forex".into())
            .spawn(move || worker(s, sink))
            .map_err(|e| format!("forex thread: {e}"))?;
        Ok(Self { state })
    }

    pub fn watch(&self, w: Watch) {
        let (lock, cvar) = &*self.state;
        let mut st = lock.lock().unwrap();
        if st.watch != w {
            st.watch = w;
            st.wake = true;
            cvar.notify_all();
        }
    }
}

impl Drop for ForexService {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.state;
        let mut st = lock.lock().unwrap();
        st.stop = true;
        st.wake = true;
        cvar.notify_all();
    }
}

fn worker(state: Arc<(Mutex<WorkerState>, Condvar)>, sink: EventSink) {
    let _com = MtaGuard::new();
    // Upstream responses can omit cache headers; never retain a Windows disk
    // cache snapshot across refreshes. Shared caching belongs to the hub.
    let http = Client::uncached(USER_AGENT).ok();
    let (lock, cvar) = &*state;
    let mut last_ask = Instant::now() - REFRESH_GAP * 2;
    let mut history_cache: HashMap<String, Instant> = HashMap::new();
    let mut latest_idr = 18133.0;
    let mut board_delivered = false;
    let mut last_seq = 0u64;

    loop {
        let watch = {
            let mut st = lock.lock().unwrap();
            while !st.stop && (!st.watch.open || (!st.wake && last_ask.elapsed() < REFRESH_GAP)) {
                let wait = if !st.watch.open {
                    Duration::from_secs(3600)
                } else {
                    REFRESH_GAP.saturating_sub(last_ask.elapsed())
                };
                st = cvar.wait_timeout(st, wait).unwrap().0;
            }
            if st.stop {
                break;
            }
            st.wake = false;
            st.watch.clone()
        };

        if !watch.open {
            continue;
        }

        let seq_changed = watch.seq != last_seq;
        if seq_changed {
            last_seq = watch.seq;
            last_ask = Instant::now() - REFRESH_GAP * 2;
            history_cache.clear();
            board_delivered = false;
        }

        let elapsed = last_ask.elapsed();
        let query_board = !board_delivered || elapsed >= REFRESH_GAP;

        if query_board {
            last_ask = Instant::now();
            let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

            if let Some(ref client) = http {
                crate::catalogs::check_remote_catalog(client, "forex", &sink);
                // 1. Primary: Centralized Edge Hub (api.dollarkilat.xyz or Cloudflare Pages)
                let mut direct_board = None;
                for endpoint in HUB_ENDPOINTS {
                    let dk_forex_url = format!("{endpoint}/v1/forex/board?symbols={}", watch.symbols.join(","));
                    // Hub may need its bounded upstream timeout plus daily FX fallback.
                    if let Ok(resp) = client.get(&dk_forex_url, Duration::from_secs(10)) {
                        if resp.status == 200 {
                            if let Ok(b) = serde_json::from_str::<ForexBoard>(&resp.body) {
                                if !b.quotes.is_empty() {
                                    direct_board = Some(b);
                                    break;
                                }
                            }
                        }
                    }
                }

                if let Some(mut board) = direct_board {
                    if let Some(q) = board.quote("USD/IDR").filter(|q| q.rate.is_finite() && q.rate > 1000.0) {
                        latest_idr = q.rate;
                    }
                    let missing = missing_live_market_symbols(&board.quotes, &watch.symbols);
                    let live_idr = board.quote("USD/IDR")
                        .map(|q| q.rate)
                        .filter(|rate| rate.is_finite() && *rate > 1000.0)
                        .or_else(|| {
                            if missing.iter().any(|symbol| symbol == "PALLADIUM") {
                                fetch_yahoo_intraday(client, "USDIDR=X")
                                    .map(|(price, _, _)| price)
                                    .filter(|rate| rate.is_finite() && *rate > 1000.0)
                            } else {
                                None
                            }
                        });
                    if let Some(rate) = live_idr {
                        latest_idr = rate;
                    }
                    let supplements = fetch_requested_market_quotes(client, &missing, live_idr, now_unix);
                    for symbol in missing {
                        if let Some(quote) = supplements.get(&symbol) {
                            board.quotes.push(quote.clone());
                        }
                    }
                    sink.send(AppEvent::Forex(board));
                    board_delivered = true;
                } else {
                    let mut rates_map = HashMap::new();
                    let mut feeds = CommodityFeeds::default();

                    // Fallback 1: open.er-api.com
                if let Ok(resp) = client.get("https://open.er-api.com/v6/latest/USD", TIMEOUT) {
                    if resp.status == 200 {
                        if let Ok(er) = serde_json::from_str::<ExchangeApiResponse>(&resp.body) {
                            rates_map = er.rates;
                        }
                    }
                }

                // 2. Fallback FX & Metals provider: jsdelivr currency-api
                if let Ok(resp) = client.get("https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json", TIMEOUT) {
                    if resp.status == 200 {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&resp.body) {
                            if let Some(usd_obj) = v.get("usd").and_then(|u| u.as_object()) {
                                if rates_map.is_empty() {
                                    for (k, val) in usd_obj {
                                        if let Some(f) = val.as_f64() {
                                            rates_map.insert(k.to_ascii_uppercase(), f);
                                        }
                                    }
                                }
                                if let Some(xau) = usd_obj.get("xau").and_then(|x| x.as_f64()).filter(|&x| x > 0.0) {
                                    feeds.gold_usd = Some((1.0 / xau, 0.0));
                                }
                                if let Some(xag) = usd_obj.get("xag").and_then(|x| x.as_f64()).filter(|&x| x > 0.0) {
                                    feeds.silver_usd = Some((1.0 / xag, 0.0));
                                }
                            }
                        }
                    }
                }

                // 3. Live Gold spot & 24h change from Binance PAXGUSDT
                if let Ok(resp) = client.get("https://data-api.binance.vision/api/v3/ticker/24hr?symbol=PAXGUSDT", TIMEOUT) {
                    if resp.status == 200 {
                        if let Ok(bt) = serde_json::from_str::<BinanceTickerResponse>(&resp.body) {
                            if let Ok(p) = bt.last_price.parse::<f64>() {
                                let pct = bt.price_change_pct.parse::<f64>().unwrap_or(0.0);
                                feeds.gold_usd = Some((p, pct));
                            }
                        }
                    }
                }

                // 4. Live Yahoo Finance spot & intraday sparklines for USD/IDR, WTI Oil, and Silver
                if let Some((price, prev, sp)) = fetch_yahoo_intraday(client, "USDIDR=X") {
                    if price > 1000.0 {
                        latest_idr = price;
                        rates_map.insert("IDR".into(), price);
                        if prev > 0.0 {
                            feeds.pcts.insert("USD/IDR".into(), (price - prev) / prev * 100.0);
                        }
                        if sp.len() >= 2 {
                            feeds.sparks.insert("USD/IDR".into(), sp);
                        }
                    }
                }
                if let Some((price, prev, sp)) = fetch_yahoo_intraday(client, "CL=F") {
                    if price > 10.0 {
                        let pct = if prev > 0.0 { (price - prev) / prev * 100.0 } else { 0.0 };
                        feeds.oil_usd = Some((price, pct));
                        if sp.len() >= 2 {
                            feeds.sparks.insert("OIL".into(), sp);
                        }
                    }
                }
                if let Some((price, prev, _)) = fetch_yahoo_intraday(client, "SI=F") {
                    if price > 5.0 {
                        let pct = if prev > 0.0 { (price - prev) / prev * 100.0 } else { 0.0 };
                        feeds.silver_usd = Some((price, pct));
                    }
                }

                if let Some(idr) = rates_map.get("IDR").copied().filter(|&x| x > 1000.0) {
                    latest_idr = idr;
                }

                let live_idr = rates_map.get("IDR").copied().filter(|&x| x.is_finite() && x > 1000.0);
                feeds.live_quotes = fetch_requested_market_quotes(client, &watch.symbols, live_idr, now_unix);

                    if let Some(board) = fallback_board(&rates_map, &feeds, &watch.symbols, now_unix) {
                        sink.send(AppEvent::Forex(board));
                    }
                    board_delivered = true;
                }
            } else {
                // Keep the last delivered board on total outage; defaults are sample data.
                board_delivered = true;
            }
        }

        // Fetch authentic 1Y daily close history for the selected chart (10m TTL or manual refresh)
        if let Some(ref sym) = watch.chart {
            let needs_fetch = seq_changed || history_cache.get(sym).map_or(true, |t| t.elapsed() > Duration::from_secs(600));
            if needs_fetch {
                if let Some(ref client) = http {
                    let mut history = None;
                    for endpoint in HUB_ENDPOINTS {
                        let dk_hist_url = format!("{endpoint}/v1/forex/history?symbol={sym}&range=1y");
                        if let Ok(resp) = client.get(&dk_hist_url, Duration::from_secs(10)) {
                            if resp.status == 200 {
                                #[derive(serde::Deserialize)]
                                struct DkForexHist {
                                    closes: Vec<(i32, f64)>,
                                }
                                if let Ok(parsed) = serde_json::from_str::<DkForexHist>(&resp.body) {
                                    if !parsed.closes.is_empty() {
                                        history = Some(parsed.closes);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if history.is_none() {
                        history = fetch_symbol_history(client, sym, latest_idr);
                    }
                    if let Some(closes) = history {
                        history_cache.insert(sym.clone(), Instant::now());
                        sink.send(AppEvent::ForexHistory(poni_core::forex::ForexHistory {
                            symbol: sym.clone(),
                            closes,
                        }));
                    }
                }
            }
        }
    }
}

fn fallback_board(rates: &HashMap<String, f64>, feeds: &CommodityFeeds, symbols: &[String], now: u64) -> Option<ForexBoard> {
    let has_rate = |code: &str| rates.get(code).is_some_and(|v| v.is_finite() && *v > 0.0);
    let valid_feed = |feed: Option<(f64, f64)>| feed.is_some_and(|(p, pct)| p.is_finite() && p > 0.0 && pct.is_finite() && pct > -100.0);
    let valid_live_quote = |symbol: &str| feeds.live_quotes.get(symbol).is_some_and(|q| {
        q.rate.is_finite() && q.rate > 0.0 && q.prev_close.is_finite() && q.prev_close > 0.0
            && q.change_pct.is_finite() && q.change_pct > -100.0
            && q.low.is_finite() && q.low > 0.0 && q.high.is_finite() && q.high >= q.low
    });
    let requested: Vec<String> = if symbols.is_empty() {
        poni_core::forex::DEFAULT_SYMBOLS.iter().map(|s| s.to_string()).collect()
    } else {
        symbols.iter().map(|s| poni_core::forex::normalize_symbol(s)).collect()
    };
    let available: Vec<String> = requested.into_iter().filter(|s| match s.as_str() {
        "GOLD" => has_rate("IDR") && valid_feed(feeds.gold_usd),
        "SILVER" => has_rate("IDR") && valid_feed(feeds.silver_usd),
        "OIL" => valid_feed(feeds.oil_usd),
        "USD/IDR" => has_rate("IDR"),
        other if valid_live_quote(other) => true,
        other => other.strip_suffix("/IDR").is_some_and(|base| has_rate(base) && has_rate("IDR")),
    }).collect();
    if available.is_empty() { return None; }
    let mut board = ForexBoard::from_rates_feeds_and_symbols(rates, feeds, &available, now);
    for q in &mut board.quotes {
        if feeds.live_quotes.contains_key(&q.symbol) {
            continue;
        }
        // The model fills demo sparklines when no observed series is supplied.
        q.spark = feeds.sparks.get(&q.symbol).cloned().unwrap_or_default();
        let pct = match q.symbol.as_str() {
            "GOLD" => feeds.gold_usd.map(|v| v.1),
            "SILVER" => feeds.silver_usd.map(|v| v.1),
            "OIL" => feeds.oil_usd.map(|v| v.1),
            other => feeds.pcts.get(other).copied(),
        }.filter(|v| v.is_finite() && *v > -100.0).unwrap_or(0.0);
        q.change_pct = pct;
        q.prev_close = q.rate / (1.0 + pct / 100.0);
        // The model's sample ranges are not observed market highs and lows.
        q.open = q.spark.first().map_or(q.rate, |p| f64::from(p.1));
        q.low = q.spark.iter().map(|p| f64::from(p.1)).fold(q.rate, f64::min);
        q.high = q.spark.iter().map(|p| f64::from(p.1)).fold(q.rate, f64::max);
    }
    Some(board)
}

fn missing_live_market_symbols(quotes: &[ForexQuote], symbols: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    symbols.iter().filter_map(|raw| {
        let symbol = poni_core::forex::normalize_symbol(raw);
        let supported = matches!(symbol.as_str(), "BRENT" | "PALLADIUM" | "COPPER" | "PALM_OIL");
        let available = quotes.iter().any(|q| q.symbol == symbol && q.rate.is_finite() && q.rate > 0.0);
        (supported && !available && seen.insert(symbol.clone())).then_some(symbol)
    }).collect()
}

fn fetch_requested_market_quotes(client: &Client, symbols: &[String], live_idr: Option<f64>, now: u64) -> HashMap<String, ForexQuote> {
    let requested: HashSet<String> = symbols.iter()
        .map(|s| poni_core::forex::normalize_symbol(s))
        .collect();
    let mut quotes = HashMap::new();
    for (symbol, ticker, name, unit) in [
        ("BRENT", "BZ=F", "Brent Crude Oil", "per barrel (USD)"),
        ("PALLADIUM", "PA=F", "Palladium Spot / g", "per 1 gram"),
        ("COPPER", "HG=F", "Copper Spot", "per lb (USD)"),
    ] {
        if !requested.contains(symbol) || (symbol == "PALLADIUM" && live_idr.is_none()) {
            continue;
        }
        let factor = if symbol == "PALLADIUM" {
            live_idr.unwrap() / poni_core::forex::TROY_OUNCE_GRAMS
        } else {
            1.0
        };
        if let Some((price, prev, spark)) = fetch_yahoo_intraday(client, ticker) {
            if let Some(quote) = make_live_quote(symbol, name, unit, factor, price, prev, &spark) {
                quotes.insert(symbol.into(), quote);
            }
        }
    }
    if requested.contains("PALM_OIL") {
        if let Some(quote) = fetch_mpob_cpo(client, now) {
            quotes.insert("PALM_OIL".into(), quote);
        }
    }
    quotes
}

fn make_live_quote(symbol: &str, name: &str, unit: &str, factor: f64, price: f64, prev: f64, spark: &[(u16, f32)]) -> Option<ForexQuote> {
    if !factor.is_finite() || factor <= 0.0 || !price.is_finite() || price <= 0.0 || !prev.is_finite() || prev <= 0.0 {
        return None;
    }
    let rate = price * factor;
    let prev_close = prev * factor;
    if !rate.is_finite() || rate <= 0.0 || !prev_close.is_finite() || prev_close <= 0.0 {
        return None;
    }
    let spark: Vec<(u16, f32)> = spark.iter().filter_map(|(time, value)| {
        let scaled = f64::from(*value) * factor;
        (scaled.is_finite() && scaled > 0.0 && scaled <= f64::from(f32::MAX))
            .then_some((*time, scaled as f32))
    }).collect();
    let change_pct = (rate / prev_close - 1.0) * 100.0;
    let baseline_low = rate.min(prev_close);
    let baseline_high = rate.max(prev_close);
    let low = spark.iter().map(|p| f64::from(p.1)).fold(baseline_low, f64::min);
    let high = spark.iter().map(|p| f64::from(p.1)).fold(baseline_high, f64::max);
    let open = spark.first().map_or(prev_close, |p| f64::from(p.1));
    Some(ForexQuote {
        symbol: symbol.into(), name: name.into(), rate, open, prev_close, low, high,
        change_pct, is_gold: false, unit: unit.into(), spark,
    })
}

fn fetch_mpob_cpo(client: &Client, now: u64) -> Option<ForexQuote> {
    let url = "https://bepi.mpob.gov.my/admin2/price_local_daily_view_cpo_msia.php?jenis=1W&more=Y";
    let response = client.get(url, TIMEOUT).ok()?;
    if response.status != 200 {
        return None;
    }
    parse_mpob_cpo(&response.body, now)
}

fn parse_mpob_cpo(html: &str, now: u64) -> Option<ForexQuote> {
    let mut plain = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    let plain = plain.replace("&nbsp;", " ").replace("&#39;", "'");
    let upper = plain.to_ascii_uppercase();
    let currency_at = upper.find("MYR")? + 3;
    let price_text: String = plain[currency_at..].trim_start().chars()
        .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
        .collect();
    let price = price_text.replace(',', "").parse::<f64>().ok()?;
    if !price.is_finite() || !(1000.0..=20000.0).contains(&price) {
        return None;
    }

    let latest = plain.split_once("Latest Price:")?.1;
    let mut date_parts = latest.split_whitespace();
    let day = date_parts.next()?.parse::<u32>().ok()?;
    let month = match date_parts.next()?.to_ascii_lowercase().as_str() {
        "january" => 1, "february" => 2, "march" => 3, "april" => 4,
        "may" => 5, "june" => 6, "july" => 7, "august" => 8,
        "september" => 9, "october" => 10, "november" => 11, "december" => 12,
        _ => return None,
    };
    let year = date_parts.next()?.parse::<i64>().ok()?;
    if !(1..=31).contains(&day) || !(1970..=2200).contains(&year) {
        return None;
    }
    let year = year - (month <= 2) as i64;
    let era = year / 400;
    let year_of_era = year - era * 400;
    let adjusted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146097 + day_of_era - 719468;
    // The page gives a date but not the revision time; use Malaysia local midnight.
    let updated_at = days_since_epoch * 86400 - 8 * 3600;
    if updated_at <= 0 || updated_at as u64 > now.saturating_add(86400)
        || now.saturating_sub(updated_at as u64) > 7 * 86400
    {
        return None;
    }

    Some(ForexQuote {
        symbol: "PALM_OIL".into(),
        name: "Crude Palm Oil (local delivered)".into(),
        rate: price,
        open: price,
        prev_close: price,
        low: price,
        high: price,
        change_pct: 0.0,
        is_gold: false,
        unit: "per tonne (MYR; daily)".into(),
        spark: Vec::new(),
    })
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    #[test]
    fn outage_does_not_replace_last_board_with_sample_prices() {
        assert!(fallback_board(&HashMap::new(), &CommodityFeeds::default(), &["GOLD".into(), "OIL".into()], 10).is_none());
    }

    #[test]
    fn partial_feeds_exclude_unavailable_commodities_and_currency_pairs() {
        let rates = HashMap::from([("IDR".into(), 16000.0), ("EUR".into(), 0.8)]);
        let feeds = CommodityFeeds { oil_usd: Some((80.0, 2.0)), ..Default::default() };
        let symbols = ["EUR/IDR", "EUR/USD", "SGD/IDR", "OIL", "GOLD", "BRENT"].map(String::from);
        let board = fallback_board(&rates, &feeds, &symbols, 10).unwrap();
        assert_eq!(board.quotes.len(), 2);
        assert_eq!(board.quote("EUR/IDR").unwrap().rate, 20000.0);
        assert_eq!(board.quote("EUR/IDR").unwrap().change_pct, 0.0);
        let eur = board.quote("EUR/IDR").unwrap();
        assert!(eur.spark.is_empty());
        assert_eq!((eur.open, eur.low, eur.high), (20000.0, 20000.0, 20000.0));
        assert_eq!(board.quote("OIL").unwrap().rate, 80.0);
    }
}

fn fetch_symbol_history(client: &Client, sym: &str, usdidr: f64) -> Option<Vec<(i32, f64)>> {
    let norm = poni_core::forex::normalize_symbol(sym);
    match norm.as_str() {
        "USD/IDR" => fetch_yahoo_daily(client, "USDIDR=X", 1.0),
        "EUR/IDR" => fetch_yahoo_daily(client, "EURIDR=X", 1.0),
        "SGD/IDR" => fetch_yahoo_daily(client, "SGDIDR=X", 1.0),
        "JPY/IDR" => fetch_yahoo_daily(client, "JPYIDR=X", 100.0),
        "MYR/IDR" => fetch_yahoo_daily(client, "MYRIDR=X", 1.0),
        "AUD/IDR" => fetch_yahoo_daily(client, "AUDIDR=X", 1.0),
        "GBP/IDR" => fetch_yahoo_daily(client, "GBPIDR=X", 1.0),
        "SAR/IDR" => fetch_yahoo_daily(client, "USDIDR=X", 1.0 / 3.75),
        "CNY/IDR" => fetch_yahoo_daily(client, "USDIDR=X", 1.0 / 7.125),
        "OIL" => fetch_yahoo_daily(client, "CL=F", 1.0),
        "BRENT" => fetch_yahoo_daily(client, "BZ=F", 1.0),
        "COPPER" => fetch_yahoo_daily(client, "HG=F", 1.0),
        "NATURAL_GAS" => fetch_yahoo_daily(client, "NG=F", 1.0),
        "PLATINUM" => {
            let factor = usdidr / poni_core::forex::TROY_OUNCE_GRAMS;
            fetch_yahoo_daily(client, "PL=F", factor)
        }
        "PALLADIUM" => {
            let factor = usdidr / poni_core::forex::TROY_OUNCE_GRAMS;
            fetch_yahoo_daily(client, "PA=F", factor)
        }
        "COFFEE" => fetch_yahoo_daily(client, "KC=F", 0.01),
        "SUGAR" => fetch_yahoo_daily(client, "SB=F", 0.01),
        "COCOA" => fetch_yahoo_daily(client, "CC=F", 1.0),
        "COTTON" => fetch_yahoo_daily(client, "CT=F", 0.01),
        "CORN" => fetch_yahoo_daily(client, "ZC=F", 0.01),
        "WHEAT" => fetch_yahoo_daily(client, "ZW=F", 0.01),
        "SOYBEANS" => fetch_yahoo_daily(client, "ZS=F", 0.01),
        "GOLD" => {
            fetch_binance_gold_history(client, usdidr).or_else(|| {
                let factor = (usdidr / poni_core::forex::TROY_OUNCE_GRAMS) * poni_core::forex::ANTAM_PREMIUM_FACTOR;
                fetch_yahoo_daily(client, "GC=F", factor)
            })
        }
        "SILVER" => {
            let factor = (usdidr / poni_core::forex::TROY_OUNCE_GRAMS) * 1.05;
            fetch_yahoo_daily(client, "SI=F", factor)
        }
        other => {
            let ticker = if other.contains('/') {
                let clean = other.replace('/', "");
                format!("{clean}=X")
            } else {
                format!("{other}=F")
            };
            fetch_yahoo_daily(client, &ticker, 1.0)
        }
    }
}

fn fetch_binance_gold_history(client: &Client, usdidr: f64) -> Option<Vec<(i32, f64)>> {
    let body = fetch_binance(client, "klines?symbol=PAXGUSDT&interval=1d&limit=365")?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let arr = json.as_array()?;
    let factor = (usdidr / poni_core::forex::TROY_OUNCE_GRAMS) * poni_core::forex::ANTAM_PREMIUM_FACTOR;
    let mut closes = Vec::with_capacity(arr.len());
    for item in arr {
        let row = item.as_array()?;
        let time_ms = row.get(0)?.as_i64()?;
        let close_str = row.get(4)?.as_str()?;
        let close_usd: f64 = close_str.parse().ok()?;
        if close_usd > 0.0 {
            let day = (time_ms / 1000 / 86400) as i32;
            closes.push((day, close_usd * factor));
        }
    }
    if closes.len() >= 10 {
        Some(closes)
    } else {
        None
    }
}

fn fetch_binance(client: &Client, path_and_query: &str) -> Option<String> {
    for base in ["https://data-api.binance.vision/api/v3", "https://api.binance.com/api/v3"] {
        let url = format!("{base}/{path_and_query}");
        if let Ok(resp) = client.get(&url, TIMEOUT) {
            if resp.status == 200 && (resp.body.starts_with('[') || resp.body.starts_with('{')) {
                return Some(resp.body);
            }
        }
    }
    None
}

fn fetch_yahoo_daily(client: &Client, ticker: &str, multiplier: f64) -> Option<Vec<(i32, f64)>> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?interval=1d&range=1y");
    let resp = client.get(&url, TIMEOUT).ok()?;
    if resp.status != 200 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&resp.body).ok()?;
    let res = v.get("chart")?.get("result")?.as_array()?.first()?;
    let timestamps = res.get("timestamp")?.as_array()?;
    let closes = res
        .get("indicators")
        .and_then(|i| i.get("quote"))
        .and_then(|q| q.as_array())
        .and_then(|a| a.first())
        .and_then(|q0| q0.get("close"))
        .and_then(|c| c.as_array())?;

    let mut out = Vec::with_capacity(timestamps.len());
    for (t, c) in timestamps.iter().zip(closes.iter()) {
        if let (Some(ts), Some(val)) = (t.as_i64(), c.as_f64()) {
            if val.is_finite() && val > 0.0 {
                let day = (ts / 86400) as i32;
                out.push((day, val * multiplier));
            }
        }
    }
    if out.len() >= 10 {
        Some(out)
    } else {
        None
    }
}

fn fetch_yahoo_intraday(client: &Client, ticker: &str) -> Option<(f64, f64, Vec<(u16, f32)>)> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}?interval=30m&range=1d");
    let resp = client.get(&url, TIMEOUT).ok()?;
    if resp.status != 200 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&resp.body).ok()?;
    let res = v.get("chart")?.get("result")?.as_array()?.first()?;
    let meta = res.get("meta")?;
    let price = meta.get("regularMarketPrice")?.as_f64()?;
    let prev = meta
        .get("chartPreviousClose")
        .or_else(|| meta.get("previousClose"))
        .and_then(|x| x.as_f64())
        .unwrap_or(price);

    let mut spark = Vec::new();
    if let Some(closes) = res
        .get("indicators")
        .and_then(|i| i.get("quote"))
        .and_then(|q| q.as_array())
        .and_then(|a| a.first())
        .and_then(|q0| q0.get("close"))
        .and_then(|c| c.as_array())
    {
        for (idx, c) in closes.iter().enumerate() {
            if let Some(val) = c.as_f64() {
                if val.is_finite() && val > 0.0 {
                    spark.push(((idx as u16) * 30, val as f32));
                }
            }
        }
    }
    Some((price, prev, spark))
}
