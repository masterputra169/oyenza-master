//! Forex & Commodities service: fetches real-time exchange rates, Gold/Silver/Oil spot prices,
//! and intraday sparklines from open.er-api.com, jsdelivr currency-api, Binance PAXG, and Yahoo Finance.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   not watched (`watch.open == false`).
//! - Free public endpoints, zero API keys.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::forex::{CommodityFeeds, ForexBoard};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) OyenzaStudio/0.2.21";
const TIMEOUT: Duration = Duration::from_secs(6);
const REFRESH_GAP: Duration = Duration::from_secs(120);
pub const DOLLARKILAT_ENDPOINT: &str = "https://api.dollarkilat.xyz";
 // 2 minutes while open

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    pub open: bool,
    pub chart: Option<String>,
    pub symbols: Vec<String>,
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
    let http = Client::new(USER_AGENT).ok();
    let (lock, cvar) = &*state;
    let mut last_ask = Instant::now() - REFRESH_GAP * 2;
    let mut history_cache: HashMap<String, Instant> = HashMap::new();
    let mut latest_idr = 18133.0;
    let mut board_delivered = false;

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

        let elapsed = last_ask.elapsed();
        let query_board = !board_delivered || elapsed >= REFRESH_GAP;

        if query_board {
            last_ask = Instant::now();
            let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

            if let Some(ref client) = http {
                // 1. Primary: Centralized Edge Hub (api.dollarkilat.xyz)
                let mut direct_board = None;
                let dk_forex_url = format!("{DOLLARKILAT_ENDPOINT}/v1/forex/board?symbols={}", watch.symbols.join(","));
                if let Ok(resp) = client.get(&dk_forex_url, TIMEOUT) {
                    if resp.status == 200 {
                        if let Ok(b) = serde_json::from_str::<ForexBoard>(&resp.body) {
                            if !b.quotes.is_empty() {
                                direct_board = Some(b);
                            }
                        }
                    }
                }

                if let Some(board) = direct_board {
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
                                    feeds.gold_usd = Some((1.0 / xau, -0.24));
                                }
                                if let Some(xag) = usd_obj.get("xag").and_then(|x| x.as_f64()).filter(|&x| x > 0.0) {
                                    feeds.silver_usd = Some((1.0 / xag, 0.45));
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
                                let pct = bt.price_change_pct.parse::<f64>().unwrap_or(-0.24);
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
                        let pct = if prev > 0.0 { (price - prev) / prev * 100.0 } else { -0.62 };
                        feeds.oil_usd = Some((price, pct));
                        if sp.len() >= 2 {
                            feeds.sparks.insert("OIL".into(), sp);
                        }
                    }
                }
                if let Some((price, prev, _)) = fetch_yahoo_intraday(client, "SI=F") {
                    if price > 5.0 {
                        let pct = if prev > 0.0 { (price - prev) / prev * 100.0 } else { 0.45 };
                        feeds.silver_usd = Some((price, pct));
                    }
                }

                if let Some(idr) = rates_map.get("IDR").copied().filter(|&x| x > 1000.0) {
                    latest_idr = idr;
                }

                    let board = if !rates_map.is_empty() {
                        if !watch.symbols.is_empty() {
                            ForexBoard::from_rates_feeds_and_symbols(&rates_map, &feeds, &watch.symbols, now_unix)
                        } else {
                            ForexBoard::from_rates_and_feeds(&rates_map, &feeds, now_unix)
                        }
                    } else {
                        ForexBoard::default_board()
                    };
                    sink.send(AppEvent::Forex(board));
                    board_delivered = true;
                }
            } else {
                sink.send(AppEvent::Forex(ForexBoard::default_board()));
                board_delivered = true;
            }
        }

        // Fetch authentic 1Y daily close history for the selected chart
        if let Some(ref sym) = watch.chart {
            let needs_fetch = history_cache.get(sym).map_or(true, |t| t.elapsed() > Duration::from_secs(300));
            if needs_fetch {
                if let Some(ref client) = http {
                    let mut history = None;
                    let dk_hist_url = format!("{DOLLARKILAT_ENDPOINT}/v1/forex/history?symbol={sym}&range=1y");
                    if let Ok(resp) = client.get(&dk_hist_url, TIMEOUT) {
                        if resp.status == 200 {
                            #[derive(serde::Deserialize)]
                            struct DkForexHist {
                                closes: Vec<(i32, f64)>,
                            }
                            if let Ok(parsed) = serde_json::from_str::<DkForexHist>(&resp.body) {
                                history = Some(parsed.closes);
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
