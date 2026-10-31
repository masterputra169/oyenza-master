//! Weather service: fetches real-time weather from Open-Meteo public API.
//!
//! Complies with the repository rule:
//! - Nothing runs at rest: the worker thread sleeps on a condition variable when
//!   not watched (`watch.open == false`).
//! - Zero API keys, completely free.
//! - Updates every 15 minutes while open/pinned.

use crate::events::{AppEvent, EventSink};
use crate::services::http::{Client, MtaGuard};
use poni_core::weather::WeatherReport;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const USER_AGENT: &str = "OyenzaStudio/0.2.21";
const TIMEOUT: Duration = Duration::from_secs(8);
const REFRESH_GAP: Duration = Duration::from_secs(900); // 15 minutes

#[derive(Clone, Debug, PartialEq)]
pub struct Watch {
    pub open: bool,
    pub lat: f64,
    pub lng: f64,
    pub city: String,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            open: false,
            lat: -6.2088,
            lng: 106.8456,
            city: "Jakarta".into(),
        }
    }
}

pub struct WeatherService {
    state: Arc<(Mutex<WorkerState>, Condvar)>,
}

#[derive(Default)]
struct WorkerState {
    watch: Watch,
    wake: bool,
    stop: bool,
}

impl WeatherService {
    pub fn start(sink: EventSink) -> Result<Self, String> {
        let state = Arc::new((Mutex::new(WorkerState::default()), Condvar::new()));
        let s = state.clone();
        std::thread::Builder::new()
            .name("weather".into())
            .spawn(move || worker(s, sink))
            .map_err(|e| format!("weather thread: {e}"))?;
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

impl Drop for WeatherService {
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
    let mut last_coords = (0.0, 0.0);

    loop {
        let (watch, was_woken) = {
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
            let woken = st.wake;
            st.wake = false;
            (st.watch.clone(), woken)
        };

        if !watch.open {
            continue;
        }

        let coords_changed = (watch.lat - last_coords.0).abs() > 0.05 || (watch.lng - last_coords.1).abs() > 0.05;
        if !was_woken && !coords_changed && last_ask.elapsed() < REFRESH_GAP {
            continue;
        }

        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,relative_humidity_2m,apparent_temperature,is_day,weather_code,wind_speed_10m",
            watch.lat, watch.lng
        );

        let now_unix = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        let mut success = false;
        if let Some(ref client) = http {
            match client.get(&url, TIMEOUT) {
                Ok(resp) => {
                    if resp.status == 200 {
                        match WeatherReport::from_json(&watch.city, &resp.body, now_unix) {
                            Ok(report) => {
                                crate::pinfo!("weather: fetched {} ({:.1}°C, {})", watch.city, report.temp, report.condition());
                                sink.send(AppEvent::Weather(report));
                                success = true;
                            }
                            Err(e) => crate::pwarn!("weather: json parse error for {}: {e}", watch.city),
                        }
                    } else {
                        crate::pwarn!("weather: HTTP status {} from {url}", resp.status);
                    }
                }
                Err(e) => crate::pwarn!("weather: HTTP fetch error from {url}: {e}"),
            }
        } else {
            crate::pwarn!("weather: no HTTP client available");
        }

        if success {
            last_coords = (watch.lat, watch.lng);
            last_ask = Instant::now();
        } else {
            // Keep last_coords as (0.0, 0.0) so next wake or 15s retry triggers
            last_coords = (0.0, 0.0);
            last_ask = Instant::now() - REFRESH_GAP + Duration::from_secs(15);
        }
    }
}
