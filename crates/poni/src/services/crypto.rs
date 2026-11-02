//! Crypto service: fetches real-time cryptocurrency data from Binance public API.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   the Crypto page is not on screen (`watch.open == false`).
//! - Offline fallback: sample data is immediately served if offline.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::crypto::{self, CryptoBoard, CryptoHistory, CryptoQuote};
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const USER_AGENT: &str = concat!("OyenzaStudio/", env!("CARGO_PKG_VERSION"), " (+https://github.com/lucacadalora/oyenza-studio)");
const TIMEOUT: Duration = Duration::from_secs(8);
const REFRESH_GAP: Duration = Duration::from_secs(15);
#[allow(dead_code)]
pub const DOLLARKILAT_ENDPOINT: &str = "https://api.dollarkilat.xyz";
pub const HUB_ENDPOINTS: &[&str] = &[
    "https://api.dollarkilat.xyz",
    "https://oyenza-master-api.pages.dev",
];


#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    pub open: bool,
    pub codes: Vec<String>,
    pub chart: Option<String>,
    pub seq: u64,
}

#[derive(Clone, Debug)]
pub enum CryptoEvent {
    Board(CryptoBoard),
    Failed(Trouble),
    History(CryptoHistory),
    NoHistory(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    Offline,
    Server,
}

pub struct CryptoService {
    state: Arc<(Mutex<WorkerState>, Condvar)>,
}

#[derive(Default)]
struct WorkerState {
    watch: Watch,
    wake: bool,
    stop: bool,
}

impl CryptoService {
    pub fn start(sink: EventSink) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(WorkerState::default()), Condvar::new()));
        let s = state.clone();
        std::thread::Builder::new()
            .name("crypto".into())
            .spawn(move || worker(s, sink))
            .map_err(|e| format!("crypto thread: {e}"))?;
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

impl Drop for CryptoService {
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
    let mut spark_cache: HashMap<String, (Instant, Vec<(u16, f32)>)> = HashMap::new();
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
            spark_cache.clear();
        }

        // 1. Fetch 24hr tickers + real 30m intraday sparklines for the watch codes
        if !watch.codes.is_empty() {
            last_ask = Instant::now();
            let mut delivered = false;
            if let Some(ref client) = http {
                crate::catalogs::check_remote_catalog(client, "crypto", &sink);
                // Try Centralized Edge Hub (api.dollarkilat.xyz or Cloudflare Pages) first
                for endpoint in HUB_ENDPOINTS {
                    let dk_url = format!("{endpoint}/v1/crypto/board?symbols={}", watch.codes.join(","));
                    if let Ok(resp) = client.get(&dk_url, Duration::from_secs(3)) {
                        if resp.status == 200 {
                            #[derive(serde::Deserialize)]
                            struct DkCryptoItem {
                                code: String,
                                price: f64,
                                prev_close: f64,
                                high: f64,
                                low: f64,
                                volume: f64,
                                #[serde(default)]
                                #[allow(dead_code)]
                                change_pct: f64,
                            }
                            #[derive(serde::Deserialize)]
                            struct DkCryptoBoard {
                                quotes: Vec<DkCryptoItem>,
                            }
                            if let Ok(b) = serde_json::from_str::<DkCryptoBoard>(&resp.body) {
                                if !b.quotes.is_empty() {
                                    let mut quotes = Vec::new();
                                    for item in b.quotes {
                                        let spark = spark_cache.get(&item.code.to_ascii_uppercase()).map(|(_, s)| s.clone()).unwrap_or_else(|| crypto::synthetic_spark(&item.code, item.prev_close, item.price, item.low, item.high));
                                        quotes.push(CryptoQuote {
                                            code: item.code.clone(),
                                            name: crypto::fallback_name(&item.code),
                                            price: item.price,
                                            prev_close: item.prev_close,
                                            open: item.prev_close,
                                            high: item.high,
                                            low: item.low,
                                            volume: item.volume,
                                            quote_volume: item.volume * item.price,
                                            market_cap: item.price * 1_000_000.0,
                                            spark,
                                        });
                                    }
                                    let now = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs() as i64)
                                        .unwrap_or(1793374000);
                                    sink.send(AppEvent::Crypto(CryptoEvent::Board(CryptoBoard {
                                        quotes,
                                        at: now,
                                        missing: Vec::new(),
                                    })));
                                    delivered = true;
                                    break;
                                }
                            }
                        }
                    }
                }

                let mut symbols = Vec::new();
                for c in &watch.codes {
                    symbols.push(format!("\"{}USDT\"", c.to_ascii_uppercase()));
                }
                let query = format!("ticker/24hr?symbols=[{}]", symbols.join(","));
                let ticker_body = if !delivered {
                    fetch_binance(client, &query).or_else(|| fetch_binance(client, "ticker/24hr"))
                } else {
                    None
                };
                if let Some(ref body) = ticker_body {
                    if let Ok(board) = parse_binance_tickers(body, &watch.codes, &spark_cache) {
                        sink.send(AppEvent::Crypto(CryptoEvent::Board(board)));
                        delivered = true;
                    }
                }

                // Fetch 30m intraday sparklines for up to 8 stale coins per cycle (TTL 120s)
                let mut updated_sparks = false;
                let mut fetched = 0usize;
                for c in &watch.codes {
                    if fetched >= 8 {
                        break;
                    }
                    let upper = c.to_ascii_uppercase();
                    let stale = spark_cache.get(&upper).map_or(true, |(t, _)| t.elapsed() > Duration::from_secs(120));
                    if stale {
                        let kurl = format!("klines?symbol={upper}USDT&interval=30m&limit=48");
                        if let Some(body) = fetch_binance(client, &kurl) {
                            if let Ok(sp) = parse_binance_intraday(&body) {
                                spark_cache.insert(upper, (Instant::now(), sp));
                                updated_sparks = true;
                                fetched += 1;
                            }
                        }
                    }
                }

                if updated_sparks {
                    if let Some(ref body) = ticker_body {
                        if let Ok(board) = parse_binance_tickers(body, &watch.codes, &spark_cache) {
                            sink.send(AppEvent::Crypto(CryptoEvent::Board(board)));
                            delivered = true;
                        }
                    }
                }
            }
            if !delivered {
                // Offline or API unreachable: deliver sample board
                sink.send(AppEvent::Crypto(CryptoEvent::Board(crypto::sample_board())));
                sink.send(AppEvent::Crypto(CryptoEvent::Failed(Trouble::Offline)));
            }
        }

        // 2. Fetch history for the picked coin if needed (10m TTL or manual refresh)
        if let Some(ref code) = watch.chart {
            let needs_fetch = seq_changed || history_cache.get(code).map_or(true, |t| t.elapsed() > Duration::from_secs(600));
            if needs_fetch {
                let symbol = format!("{}USDT", code.to_ascii_uppercase());
                let query = format!("klines?symbol={symbol}&interval=1d&limit=365");
                let mut delivered = false;
                if let Some(ref client) = http {
                    // Try Central Edge Hub first (covers HYPE / Gate.io and bypasses firewall blocks)
                    for endpoint in HUB_ENDPOINTS {
                        let dk_url = format!("{endpoint}/v1/crypto/history?symbol={code}&range=1y");
                        if let Ok(resp) = client.get(&dk_url, Duration::from_secs(3)) {
                            if resp.status == 200 {
                                #[derive(serde::Deserialize)]
                                struct DkCryptoPoint {
                                    time: i64,
                                    close: f64,
                                }
                                #[derive(serde::Deserialize)]
                                struct DkCryptoHist {
                                    points: Vec<DkCryptoPoint>,
                                }
                                if let Ok(parsed) = serde_json::from_str::<DkCryptoHist>(&resp.body) {
                                    if !parsed.points.is_empty() {
                                        let closes: Vec<(i32, f64)> = parsed.points.into_iter().map(|p| ((p.time / 86400) as i32, p.close)).collect();
                                        history_cache.insert(code.clone(), Instant::now());
                                        sink.send(AppEvent::Crypto(CryptoEvent::History(CryptoHistory {
                                            code: code.clone(),
                                            closes,
                                        })));
                                        delivered = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if !delivered {
                        if let Some(body) = fetch_binance(client, &query) {
                            if let Ok(hist) = parse_binance_klines(code, &body) {
                                history_cache.insert(code.clone(), Instant::now());
                                sink.send(AppEvent::Crypto(CryptoEvent::History(hist)));
                                delivered = true;
                            }
                        }
                    }
                }
                if !delivered {
                    sink.send(AppEvent::Crypto(CryptoEvent::NoHistory(code.clone())));
                }
            }
        }
    }
}

/// Fetches from Binance's public market data CDN (`data-api.binance.vision`, unblocked globally),
/// with fallback to `api.binance.com`.
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

fn parse_binance_intraday(body: &str) -> Result<Vec<(u16, f32)>, ()> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|_| ())?;
    let arr = json.as_array().ok_or(())?;
    let mut spark = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        if let Some(row) = item.as_array() {
            if let Some(close_val) = row.get(4).and_then(|v| v.as_str()).and_then(|s| s.parse::<f32>().ok()) {
                if close_val.is_finite() && close_val > 0.0 {
                    spark.push(((i as u16) * 30, close_val));
                }
            }
        }
    }
    if spark.len() < 2 {
        return Err(());
    }
    Ok(spark)
}

fn parse_binance_tickers(
    body: &str,
    codes: &[String],
    spark_cache: &HashMap<String, (Instant, Vec<(u16, f32)>)>,
) -> Result<CryptoBoard, ()> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|_| ())?;
    let arr = json.as_array().ok_or(())?;
    let mut quotes = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(1793374000);

    for val in arr {
        let sym = val.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
        if !sym.ends_with("USDT") {
            continue;
        }
        let code = sym.trim_end_matches("USDT").to_string();
        if !codes.iter().any(|c| c.eq_ignore_ascii_case(&code)) {
            continue;
        }
        let price: f64 = val.get("lastPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let prev_close: f64 = val.get("prevClosePrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(price);
        let open: f64 = val.get("openPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(price);
        let high: f64 = val.get("highPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(price);
        let low: f64 = val.get("lowPrice").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(price);
        let volume: f64 = val.get("volume").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let quote_volume: f64 = val.get("quoteVolume").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let name = crypto::coin_name(&code).to_string();
        let market_cap = approx_supply(&code)
            .map(|supply| price * supply)
            .unwrap_or_else(|| quote_volume * 30.0);

        let mut spark = spark_cache
            .get(&code.to_ascii_uppercase())
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| crypto::synthetic_spark(&code, open, price, low, high));
        if let Some(last) = spark.last_mut() {
            last.1 = price as f32;
        }

        let final_name = if !name.is_empty() { name } else { crypto::fallback_name(&code) };

        quotes.push(CryptoQuote {
            code,
            name: final_name,
            price,
            prev_close,
            open,
            high,
            low,
            volume,
            quote_volume,
            market_cap,
            spark,
        });
    }

    if quotes.is_empty() {
        return Err(());
    }
    Ok(CryptoBoard { at: now, quotes, missing: Vec::new() })
}

fn parse_binance_klines(code: &str, body: &str) -> Result<CryptoHistory, ()> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|_| ())?;
    let arr = json.as_array().ok_or(())?;
    let mut closes = Vec::new();
    let today = crypto::today_num();

    for (i, item) in arr.iter().enumerate() {
        if let Some(row) = item.as_array() {
            let day = row
                .first()
                .and_then(|v| v.as_i64())
                .map(|ms| (ms / 86_400_000) as i32)
                .unwrap_or_else(|| today - (arr.len() - 1 - i) as i32);
            if let Some(close_val) = row.get(4).and_then(|v| v.as_str()).and_then(|s| s.parse::<f64>().ok()) {
                if close_val.is_finite() && close_val > 0.0 {
                    closes.push((day, close_val));
                }
            }
        }
    }
    if closes.is_empty() {
        return Err(());
    }
    Ok(CryptoHistory { code: code.to_string(), closes })
}

/// Circulating supply estimates (based on CoinMarketCap figures) used to compute
/// real-time market cap (`price * supply`) for Top 100+ Binance-listed coins.
fn approx_supply(code: &str) -> Option<f64> {
    let s = match code {
        "BTC" => 19.8e6,
        "ETH" => 120.5e6,
        "XRP" => 57.8e9,
        "BNB" => 142.5e6,
        "SOL" => 488.0e6,
        "DOGE" => 148.0e9,
        "ADA" => 35.9e9,
        "TRX" => 86.1e9,
        "HYPE" => 239.5e6,
        "TON" => 2.5e9,
        "LINK" => 638.0e6,
        "AVAX" => 414.0e6,
        "SUI" => 3.1e9,
        "XLM" => 30.6e9,
        "WBTC" => 129.0e3,
        "SHIB" => 589.2e12,
        "HBAR" => 41.9e9,
        "BCH" => 19.8e6,
        "DOT" => 1.54e9,
        "LTC" => 75.5e6,
        "UNI" => 600.5e6,
        "NEAR" => 1.18e9,
        "PEPE" => 420.69e12,
        "APT" => 588.0e6,
        "ICP" => 481.0e6,
        "AAVE" => 15.0e6,
        "ETC" => 150.8e6,
        "ONDO" => 3.16e9,
        "TAO" => 8.4e6,
        "RENDER" => 517.7e6,
        "POL" => 8.58e9,
        "VET" => 80.98e9,
        "FIL" => 632.0e6,
        "ARB" => 4.4e9,
        "FET" => 2.6e9,
        "TRUMP" => 200.0e6,
        "OM" => 965.0e6,
        "ALGO" => 8.43e9,
        "ATOM" => 390.9e6,
        "OP" => 1.62e9,
        "ENA" => 5.28e9,
        "STX" => 1.51e9,
        "INJ" => 99.0e6,
        "TIA" => 537.0e6,
        "IMX" => 1.76e9,
        "GRT" => 9.55e9,
        "BONK" => 77.4e12,
        "WLD" => 1.07e9,
        "S" => 2.88e9,
        "SEI" => 4.65e9,
        "THETA" => 1.0e9,
        "WIF" => 998.8e6,
        "FLOKI" => 9.6e12,
        "JASMY" => 49.4e9,
        "LDO" => 897.0e6,
        "SAND" => 2.45e9,
        "GALA" => 37.0e9,
        "QNT" => 14.54e6,
        "JUP" => 2.69e9,
        "PYTH" => 3.62e9,
        "KAIA" => 5.85e9,
        "FLOW" => 1.56e9,
        "XTZ" => 1.03e9,
        "IOTA" => 3.65e9,
        "BEAMX" => 49.5e9,
        "NEO" => 70.5e6,
        "AR" => 65.65e6,
        "CRV" => 1.28e9,
        "AXS" => 159.0e6,
        "MANA" => 1.94e9,
        "EGLD" => 27.9e6,
        "PENDLE" => 164.0e6,
        "STRK" => 2.58e9,
        "DYDX" => 728.0e6,
        "RAY" => 290.8e6,
        "CAKE" => 291.0e6,
        "ENS" => 33.16e6,
        "ZEC" => 16.32e6,
        "CHZ" => 9.35e9,
        "PAXG" => 185.0e3,
        "CFX" => 4.92e9,
        "APE" => 752.6e6,
        "RUNE" => 351.0e6,
        "MINA" => 1.21e9,
        "SNX" => 339.8e6,
        "COMP" => 8.87e6,
        "ZK" => 3.67e9,
        "EIGEN" => 244.5e6,
        "PENGU" => 62.86e9,
        "VIRTUAL" => 645.0e6,
        "BERA" => 107.5e6,
        "MOVE" => 2.4e9,
        "NEXO" => 646.0e6,
        "DASH" => 12.15e6,
        "KAVA" => 1.08e9,
        "1INCH" => 1.4e9,
        "W" => 2.86e9,
        "CKB" => 45.8e9,
        "ROSE" => 7.06e9,
        "BLUR" => 2.21e9,
        "JTO" => 297.0e6,
        "ZRO" => 111.0e6,
        "ORDI" => 21.0e6,
        "NOT" => 102.4e9,
        "WOO" => 1.86e9,
        "ASTR" => 7.57e9,
        "GMT" => 2.62e9,
        "ZIL" => 19.2e9,
        "CELO" => 561.0e6,
        "ANKR" => 10.0e9,
        "IOTX" => 9.44e9,
        "QTUM" => 105.5e6,
        "KSM" => 15.8e6,
        "RSR" => 55.3e9,
        "BAT" => 1.49e9,
        "ZRX" => 848.4e6,
        "ENJ" => 1.8e9,
        "LRC" => 1.37e9,
        "HOT" => 174.5e9,
        "RVN" => 14.8e9,
        "SUSHI" => 261.0e6,
        "YFI" => 33.6e3,
        "MKR" => 880.0e3,
        "SKY" => 21.3e9,
        "AERO" => 800.0e6,
        "MORPHO" => 236.0e6,
        "ETHFI" => 231.0e6,
        "GNO" => 2.59e6,
        "KAITO" => 241.4e6,
        "SYRUP" => 1.07e9,
        "WLFI" => 20.0e9,
        "LINEA" => 15.8e9,
        "PUMP" => 354.0e9,
        "ASTER" => 1.65e9,
        "PLUME" => 2.0e9,
        "KMNO" => 1.35e9,
        "ORCA" => 53.0e6,
        "PNUT" => 999.8e6,
        _ => return None,
    };
    Some(s)
}
