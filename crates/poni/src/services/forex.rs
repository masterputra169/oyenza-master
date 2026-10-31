//! Forex & Gold service: fetches exchange rates and physical gold spot.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   not watched (`watch.open == false`).
//! - Free public endpoints, zero API keys.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::forex::ForexBoard;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = concat!("OyenzaStudio/", env!("CARGO_PKG_VERSION"), " (+https://github.com/masterputra169/oyenza-master)");
const TIMEOUT: Duration = Duration::from_secs(8);
const REFRESH_GAP: Duration = Duration::from_secs(300); // 5 minutes

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
            if let Ok(resp) = client.get("https://open.er-api.com/v6/latest/USD", TIMEOUT) {
                if resp.status == 200 {
                    if let Ok(er) = serde_json::from_str::<ExchangeApiResponse>(&resp.body) {
                        rates_map = er.rates;
                    }
                }
            }

            let mut gold_usd = None;
            if let Ok(resp) = client.get("https://data-api.binance.vision/api/v3/ticker/24hr?symbol=PAXGUSDT", TIMEOUT) {
                if resp.status == 200 {
                    if let Ok(bt) = serde_json::from_str::<BinanceTickerResponse>(&resp.body) {
                        if let Ok(p) = bt.last_price.parse::<f64>() {
                            gold_usd = Some(p);
                        }
                    }
                }
            }

            if !rates_map.is_empty() {
                let board = ForexBoard::from_rates(&rates_map, gold_usd, now_unix);
                sink.send(AppEvent::Forex(board));
            }
        }
    }
}
