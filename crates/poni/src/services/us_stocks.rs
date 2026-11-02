//! US Stocks service: fetches real-time quotes, intraday sparklines, and 1-year historical
//! daily closes for top US equities from Yahoo Finance (/v8/finance/chart).
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   not watched (`watch.open == false`).
//! - Free public endpoints, zero API keys.
//! - Refresh every 60 seconds while open.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::us_stocks::{self, UsStockBoard, UsStockQuote};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) OyenzaStudio/0.2.22";
const TIMEOUT: Duration = Duration::from_secs(6);
const REFRESH_GAP: Duration = Duration::from_secs(60);
#[allow(dead_code)]
pub const DOLLARKILAT_ENDPOINT: &str = "https://api.dollarkilat.xyz";
pub const HUB_ENDPOINTS: &[&str] = &[
    "https://api.dollarkilat.xyz",
    "https://oyenza-master-api.pages.dev",
];
 // 60 seconds while open

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    pub open: bool,
    pub chart: Option<String>,
    pub codes: Vec<String>,
    pub seq: u64,
}

pub struct UsStocksService {
    state: Arc<(Mutex<WorkerState>, Condvar)>,
}

#[derive(Default)]
struct WorkerState {
    watch: Watch,
    wake: bool,
    stop: bool,
}

impl UsStocksService {
    pub fn start(sink: EventSink) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(WorkerState::default()), Condvar::new()));
        let s = state.clone();
        std::thread::Builder::new()
            .name("us_stocks".into())
            .spawn(move || worker(s, sink))
            .map_err(|e| format!("us_stocks thread: {e}"))?;
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

impl Drop for UsStocksService {
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

        let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        let codes = if !watch.codes.is_empty() {
            watch.codes.clone()
        } else {
            us_stocks::STARTERS.iter().map(|&(s, _)| s.to_string()).collect()
        };

        // Fetch quotes for requested watchlist US stocks
        if let Some(ref client) = http {
            let mut quotes = Vec::new();

            // 1. Primary: Centralized Edge Hub (api.dollarkilat.xyz or Cloudflare Pages)
            for endpoint in HUB_ENDPOINTS {
                let dk_url = format!("{endpoint}/v1/us-stocks/quotes?symbols={}", codes.join(","));
                if let Ok(resp) = client.get(&dk_url, Duration::from_secs(3)) {
                    if resp.status == 200 {
                        #[derive(serde::Deserialize)]
                        struct DkUsBoard {
                            quotes: Vec<UsStockQuote>,
                        }
                        if let Ok(board) = serde_json::from_str::<DkUsBoard>(&resp.body) {
                            if !board.quotes.is_empty() {
                                quotes = board.quotes;
                                break;
                            }
                        }
                    }
                }
            }

            // 2. Fallback: Direct Yahoo Finance if edge server not yet deployed or down
            if quotes.is_empty() {
                for sym in &codes {
                    let sym_upper = sym.to_ascii_uppercase();
                    let default_name = us_stocks::name_for(&sym_upper).to_string();
                    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{sym_upper}?interval=5m&range=1d");
                    if let Ok(resp) = client.get(&url, TIMEOUT) {
                        if resp.status == 200 {
                            if let Ok(chart) = us_stocks::parse_yahoo_chart_json(&resp.body) {
                                if chart.price > 0.0 {
                                    let name = chart.short_name.unwrap_or(default_name);
                                    quotes.push(UsStockQuote {
                                        symbol: sym_upper,
                                        name,
                                        price: chart.price,
                                        prev: chart.prev,
                                        high: chart.high,
                                        low: chart.low,
                                        volume: chart.volume,
                                        w52: chart.w52,
                                        spark: chart.spark,
                                        open: chart.open,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            if !quotes.is_empty() {
                sink.send(AppEvent::UsStocks(UsStockBoard {
                    quotes,
                    updated_at: now_unix,
                }));
                board_delivered = true;
                last_ask = Instant::now();
            } else if !board_delivered {
                // Initial fallback board if network failed
                sink.send(AppEvent::UsStocks(UsStockBoard::default_board()));
                board_delivered = true;
            }

            // Historical daily data for selected stock (10m TTL or manual refresh)
            if let Some(ref pick) = watch.chart {
                let should_fetch = seq_changed || history_cache.get(pick).map_or(true, |t| t.elapsed() > Duration::from_secs(600));
                if should_fetch {
                    let mut points = Vec::new();
                    for endpoint in HUB_ENDPOINTS {
                        let dk_hist_url = format!("{endpoint}/v1/us-stocks/history?symbol={pick}&range=1y");
                        if let Ok(resp) = client.get(&dk_hist_url, Duration::from_secs(3)) {
                            if resp.status == 200 {
                                #[derive(serde::Deserialize)]
                                struct DkHist {
                                    closes: Vec<(i32, f64)>,
                                }
                                if let Ok(parsed) = serde_json::from_str::<DkHist>(&resp.body) {
                                    if !parsed.closes.is_empty() {
                                        points = parsed.closes;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if points.is_empty() {
                        let hist_url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{pick}?interval=1d&range=1y");
                        if let Ok(resp) = client.get(&hist_url, TIMEOUT) {
                            if resp.status == 200 {
                                if let Ok(pts) = us_stocks::parse_yahoo_history_json(&resp.body) {
                                    points = pts;
                                }
                            }
                        }
                    }
                    if !points.is_empty() {
                        history_cache.insert(pick.clone(), Instant::now());
                        sink.send(AppEvent::UsStocksHistory {
                            symbol: pick.clone(),
                            points,
                        });
                    }
                }
            }
        } else if !board_delivered {
            sink.send(AppEvent::UsStocks(UsStockBoard::default_board()));
            board_delivered = true;
        }
    }
}
