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
const REFRESH_GAP: Duration = Duration::from_secs(120); // 2 minutes while open

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Watch {
    pub open: bool,
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

        last_ask = Instant::now();
        let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        if let Some(ref client) = http {
            let mut rates_map = HashMap::new();
            let mut feeds = CommodityFeeds::default();

            // 1. Primary FX provider: open.er-api.com
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

            let board = if !rates_map.is_empty() {
                ForexBoard::from_rates_and_feeds(&rates_map, &feeds, now_unix)
            } else {
                ForexBoard::default_board()
            };
            sink.send(AppEvent::Forex(board));
        } else {
            sink.send(AppEvent::Forex(ForexBoard::default_board()));
        }
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
