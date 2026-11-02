//! Dynamic external catalogs for Crypto, US Stocks, and Forex/Commodities.
//!
//! Large symbol catalogs are hosted remotely (e.g. Cloudflare Pages) and
//! cached locally under `%LOCALAPPDATA%\Poni\catalogs\`, keeping the
//! `poni.exe` binary ultra-light while allowing instant, offline autocomplete.

use std::fs;
use std::path::Path;
use serde::Deserialize;

#[derive(Clone, Debug, Default)]
pub struct CatalogsState {
    pub crypto: Vec<(String, String)>,
    pub us_stocks: Vec<(String, String)>,
    pub forex: Vec<(String, String)>,
    pub loaded: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CatalogEntry {
    Pair(String, String),
    Obj { code: String, name: String },
    SymObj { symbol: String, name: String },
}

impl CatalogEntry {
    fn into_pair(self) -> (String, String) {
        match self {
            Self::Pair(c, n) => (c, n),
            Self::Obj { code, name } => (code, name),
            Self::SymObj { symbol, name } => (symbol, name),
        }
    }
}

fn load_file(path: &Path) -> Vec<(String, String)> {
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<CatalogEntry>>(&data) else {
        return Vec::new();
    };
    let mut list = Vec::with_capacity(entries.len());
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        let (c, n) = entry.into_pair();
        let upper = c.trim().to_ascii_uppercase();
        if !upper.is_empty() && seen.insert(upper.clone()) {
            list.push((upper, n.trim().to_string()));
        }
    }
    list
}

pub fn save_catalog(path: &Path, items: &[(String, String)]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(items) {
        let _ = fs::write(path, json);
    }
}

impl CatalogsState {
    pub fn load_from_disk(&mut self) {
        if self.loaded {
            return;
        }
        self.crypto = load_file(&crate::paths::crypto_catalog_path());
        self.us_stocks = load_file(&crate::paths::us_stocks_catalog_path());
        self.forex = load_file(&crate::paths::forex_catalog_path());
        self.loaded = true;
    }

    pub fn search_crypto<'a>(&'a self, query: &str, skip: &[String]) -> Vec<(&'a str, &'a str)> {
        let q = query.trim().to_ascii_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        if !self.crypto.is_empty() {
            rank_matches(&q, skip, &self.crypto)
        } else {
            poni_core::crypto::search_coins(query, skip)
        }
    }

    pub fn search_us_stocks<'a>(&'a self, query: &str, skip: &[String]) -> Vec<(&'a str, &'a str)> {
        let q = query.trim().to_ascii_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        if !self.us_stocks.is_empty() {
            rank_matches(&q, skip, &self.us_stocks)
        } else {
            poni_core::us_stocks::search_us_stocks(query, skip)
        }
    }

    pub fn search_forex<'a>(&'a self, query: &str, skip: &[String]) -> Vec<(&'a str, &'a str)> {
        let q = query.trim().to_ascii_uppercase();
        if q.is_empty() {
            return Vec::new();
        }
        if !self.forex.is_empty() {
            rank_matches(&q, skip, &self.forex)
        } else {
            poni_core::forex::search_forex(query, skip)
        }
    }

    pub fn crypto_name<'a>(&'a self, code: &str) -> &'a str {
        let upper = code.trim().to_ascii_uppercase();
        for (c, n) in &self.crypto {
            if *c == upper {
                return n.as_str();
            }
        }
        let static_name = poni_core::crypto::coin_name(code);
        if !static_name.is_empty() {
            static_name
        } else {
            ""
        }
    }

    pub fn us_stock_name<'a>(&'a self, code: &str) -> &'a str {
        let upper = code.trim().to_ascii_uppercase();
        for (c, n) in &self.us_stocks {
            if *c == upper {
                return n.as_str();
            }
        }
        poni_core::us_stocks::name_for(code)
    }

    pub fn forex_name<'a>(&'a self, symbol: &str) -> &'a str {
        let upper = symbol.trim().to_ascii_uppercase();
        for (c, n) in &self.forex {
            if *c == upper {
                return n.as_str();
            }
        }
        for &(s, n) in &poni_core::forex::KNOWN_FOREX_ITEMS {
            if s.eq_ignore_ascii_case(symbol) {
                return n;
            }
        }
        ""
    }
}

fn rank_matches<'a>(q: &str, skip: &[String], catalog: &'a [(String, String)]) -> Vec<(&'a str, &'a str)> {
    let mut ranked: Vec<(u8, usize, &'a str, &'a str)> = Vec::new();
    for (idx, (c, n)) in catalog.iter().enumerate() {
        if skip.iter().any(|x| x.eq_ignore_ascii_case(c)) {
            continue;
        }
        let nu = n.to_ascii_uppercase();
        let rank = if c == q {
            0
        } else if c.starts_with(q) {
            1
        } else if nu == q {
            2
        } else if nu.starts_with(q) {
            3
        } else if nu.split_whitespace().any(|w| w.starts_with(q)) {
            4
        } else if c.contains(q) {
            5
        } else if nu.contains(q) {
            6
        } else {
            continue;
        };
        ranked.push((rank, idx, c.as_str(), n.as_str()));
    }
    ranked.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    ranked.into_iter().take(40).map(|(_, _, c, n)| (c, n)).collect()
}

pub fn check_remote_catalog(
    client: &crate::services::http::Client,
    kind: &'static str,
    sink: &crate::events::EventSink,
) {
    let endpoints = &[
        "https://api.dollarkilat.xyz",
        "https://oyenza-master-api.pages.dev",
    ];
    let path = match kind {
        "crypto" => crate::paths::crypto_catalog_path(),
        "us_stocks" => crate::paths::us_stocks_catalog_path(),
        "forex" => crate::paths::forex_catalog_path(),
        _ => return,
    };
    if let Ok(meta) = std::fs::metadata(&path) {
        if let Ok(mtime) = meta.modified() {
            if let Ok(elapsed) = mtime.elapsed() {
                if elapsed < std::time::Duration::from_secs(7 * 86400) {
                    return;
                }
            }
        }
    }
    for endpoint in endpoints {
        let url = format!("{endpoint}/catalogs/{kind}.json");
        if let Ok(resp) = client.get(&url, std::time::Duration::from_secs(4)) {
            if resp.status == 200 {
                if let Ok(entries) = serde_json::from_str::<Vec<CatalogEntry>>(&resp.body) {
                    if !entries.is_empty() {
                        let mut list = Vec::with_capacity(entries.len());
                        let mut seen = std::collections::HashSet::new();
                        for entry in entries {
                            let (c, n) = entry.into_pair();
                            let upper = c.trim().to_ascii_uppercase();
                            if !upper.is_empty() && seen.insert(upper.clone()) {
                                list.push((upper, n.trim().to_string()));
                            }
                        }
                        save_catalog(&path, &list);
                        sink.send(crate::events::AppEvent::CatalogUpdated { kind, items: list });
                        return;
                    }
                }
            }
        }
    }
}
