/**
 * DollarKilat Market Data Aggregator & Edge Cache
 * Platform: Cloudflare Pages Functions
 * Route: functions/[[path]].js
 */

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, HEAD, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, User-Agent, X-Oyenza-Version",
  "Content-Type": "application/json; charset=utf-8",
};

const USER_AGENT = "DollarKilat-Hub/1.0 (OyenzaMaster; Cloudflare Pages)";

const FOREX_CATALOG = [
  { code: "USD/IDR", name: "US Dollar / Indonesian Rupiah", unit: "per 1 USD" },
  { code: "EUR/IDR", name: "Euro / Indonesian Rupiah", unit: "per 1 EUR" },
  { code: "SGD/IDR", name: "Singapore Dollar / Rupiah", unit: "per 1 SGD" },
  { code: "JPY/IDR", name: "Japanese Yen / Rupiah", unit: "per 100 JPY" },
  { code: "SAR/IDR", name: "Saudi Riyal / Indonesian Rupiah", unit: "per 1 SAR" },
  { code: "MYR/IDR", name: "Malaysian Ringgit / Rupiah", unit: "per 1 MYR" },
  { code: "CNY/IDR", name: "Chinese Yuan / Rupiah", unit: "per 1 CNY" },
  { code: "AUD/IDR", name: "Australian Dollar / Rupiah", unit: "per 1 AUD" },
  { code: "GBP/IDR", name: "British Pound / Rupiah", unit: "per 1 GBP" },
  { code: "CHF/IDR", name: "Swiss Franc / Rupiah", unit: "per 1 CHF" },
  { code: "CAD/IDR", name: "Canadian Dollar / Rupiah", unit: "per 1 CAD" },
  { code: "HKD/IDR", name: "Hong Kong Dollar / Rupiah", unit: "per 1 HKD" },
  { code: "KRW/IDR", name: "South Korean Won / Rupiah", unit: "per 100 KRW" },
  { code: "THB/IDR", name: "Thai Baht / Indonesian Rupiah", unit: "per 1 THB" },
  { code: "NZD/IDR", name: "New Zealand Dollar / Rupiah", unit: "per 1 NZD" },
  { code: "TWD/IDR", name: "New Taiwan Dollar / Rupiah", unit: "per 1 TWD" },
  { code: "INR/IDR", name: "Indian Rupee / Indonesian Rupiah", unit: "per 1 INR" },
  { code: "AED/IDR", name: "UAE Dirham / Indonesian Rupiah", unit: "per 1 AED" },
  { code: "TRY/IDR", name: "Turkish Lira / Indonesian Rupiah", unit: "per 1 TRY" },
  { code: "PHP/IDR", name: "Philippine Peso / Rupiah", unit: "per 1 PHP" },
  { code: "VND/IDR", name: "Vietnamese Dong / Rupiah", unit: "per 1000 VND" },
  { code: "BND/IDR", name: "Brunei Dollar / Rupiah", unit: "per 1 BND" },
  { code: "QAR/IDR", name: "Qatari Riyal / Rupiah", unit: "per 1 QAR" },
  { code: "KWD/IDR", name: "Kuwaiti Dinar / Rupiah", unit: "per 1 KWD" },
  { code: "BRL/IDR", name: "Brazilian Real / Rupiah", unit: "per 1 BRL" },
  { code: "MXN/IDR", name: "Mexican Peso / Rupiah", unit: "per 1 MXN" },
  { code: "ZAR/IDR", name: "South African Rand / Rupiah", unit: "per 1 ZAR" },
  { code: "SEK/IDR", name: "Swedish Krona / Rupiah", unit: "per 1 SEK" },
  { code: "NOK/IDR", name: "Norwegian Krone / Rupiah", unit: "per 1 NOK" },
  { code: "DKK/IDR", name: "Danish Krone / Rupiah", unit: "per 1 DKK" },
  { code: "PLN/IDR", name: "Polish Zloty / Rupiah", unit: "per 1 PLN" },
  { code: "RUB/IDR", name: "Russian Ruble / Rupiah", unit: "per 1 RUB" },

  // Commodities
  { code: "GOLD", name: "Gold Spot / Emas Antam (IDR)", unit: "per 1 gram" },
  { code: "SILVER", name: "Silver Spot / Perak (IDR)", unit: "per 1 gram" },
  { code: "OIL", name: "WTI Crude Oil (USD)", unit: "per barrel (USD)" },
  { code: "BRENT", name: "Brent Crude Oil (USD)", unit: "per barrel (USD)" },
  { code: "PLATINUM", name: "Platinum Spot / Platina (IDR)", unit: "per 1 gram" },
  { code: "PALLADIUM", name: "Palladium Spot (IDR)", unit: "per 1 gram" },
  { code: "NATURAL_GAS", name: "Natural Gas (USD)", unit: "per MMBtu (USD)" },
  { code: "COPPER", name: "Copper Spot / Tembaga (USD)", unit: "per lb (USD)" },
  { code: "CPO", name: "Crude Palm Oil / Minyak Sawit", unit: "per ton (MYR/USD)" },
  { code: "NICKEL", name: "Nickel Spot / Nikel (USD)", unit: "per ton (USD)" },
  { code: "COAL", name: "Newcastle Coal / Batu Bara (USD)", unit: "per ton (USD)" },
  { code: "WHEAT", name: "Wheat Futures / Gandum (USD)", unit: "per bushel (USD)" },
  { code: "COFFEE", name: "Coffee Futures / Kopi Arabica (USD)", unit: "per lb (USD)" },
];

export async function onRequest(context) {
  const { request, next } = context;
  if (request.method === "OPTIONS") {
    return new Response(null, { headers: CORS_HEADERS });
  }

  const url = new URL(request.url);
  const path = url.pathname.replace(/\/+$/, "");

  try {
    // 1. Health check & Metadata
    if (path === "/health" || path === "/v1") {
      return jsonResponse({
        status: "ok",
        service: "DollarKilat Market Hub",
        domain: "api.dollarkilat.xyz",
        time: new Date().toISOString(),
        version: "1.0.0",
        endpoints: [
          "/v1/us-stocks/quotes?symbols=AAPL,NVDA,TSLA",
          "/v1/us-stocks/search?q=apple",
          "/v1/us-stocks/history?symbol=AAPL&range=1y",
          "/v1/forex/board?symbols=USD/IDR,GOLD,OIL",
          "/v1/forex/search?q=euro",
          "/v1/forex/history?symbol=USDIDR=X&range=1y",
          "/v1/crypto/board?symbols=BTC,ETH,SOL",
          "/v1/crypto/search?q=btc",
        ],
      }, 10);
    }

    // 2. US Stocks Quotes
    if (path === "/v1/us-stocks/quotes") {
      const symbolsParam = url.searchParams.get("symbols") || "AAPL,NVDA,MSFT,AMZN,GOOGL,META,TSLA";
      const symbols = symbolsParam.split(",").map((s) => s.trim().toUpperCase()).filter(Boolean);
      const data = await handleUsStockQuotes(symbols);
      return jsonResponse(data, 15);
    }

    // 3. US Stocks Search
    if (path === "/v1/us-stocks/search") {
      const query = url.searchParams.get("q") || "";
      const data = await handleUsStockSearch(query);
      return jsonResponse(data, 3600);
    }

    // 4. US Stocks Historical Chart
    if (path === "/v1/us-stocks/history") {
      const symbol = (url.searchParams.get("symbol") || "AAPL").trim().toUpperCase();
      const range = (url.searchParams.get("range") || "1y").toLowerCase();
      const data = await handleUsStockHistory(symbol, range);
      return jsonResponse(data, 300);
    }

    // 5. Forex & Commodities Board
    if (path === "/v1/forex/board") {
      const symbolsParam = url.searchParams.get("symbols") || "USD/IDR,EUR/IDR,SGD/IDR,JPY/IDR,GOLD,OIL";
      const symbols = symbolsParam.split(",").map((s) => s.trim().toUpperCase()).filter(Boolean);
      const data = await handleForexBoard(symbols);
      return jsonResponse(data, 30);
    }

    // 6. Forex & Commodities Search
    if (path === "/v1/forex/search") {
      const query = url.searchParams.get("q") || "";
      const data = handleForexSearch(query);
      return jsonResponse(data, 86400);
    }

    // 7. Forex & Commodities 1Y History
    if (path === "/v1/forex/history") {
      const symbol = url.searchParams.get("symbol") || "USD/IDR";
      const data = await handleForexHistory(symbol);
      return jsonResponse(data, 600);
    }

    // 8. Crypto Board
    if (path === "/v1/crypto/board") {
      const symbolsParam = url.searchParams.get("symbols") || "BTC,ETH,SOL,BNB,XRP,DOGE,ADA";
      const symbols = symbolsParam.split(",").map((s) => s.trim().toUpperCase()).filter(Boolean);
      const data = await handleCryptoBoard(symbols);
      return jsonResponse(data, 15);
    }

    // 9. Crypto Search
    if (path === "/v1/crypto/search") {
      const query = url.searchParams.get("q") || "";
      const data = await handleCryptoSearch(query);
      return jsonResponse(data, 3600);
    }

    if (next) {
      return await next();
    }

    return new Response(JSON.stringify({ error: "Endpoint not found", path }), {
      status: 404,
      headers: CORS_HEADERS,
    });
  } catch (err) {
    return new Response(JSON.stringify({ error: err.message, stack: err.stack }), {
      status: 500,
      headers: CORS_HEADERS,
    });
  }
}

function jsonResponse(data, maxAgeSeconds = 0) {
  const headers = { ...CORS_HEADERS };
  if (maxAgeSeconds > 0) {
    headers["Cache-Control"] = `public, max-age=${maxAgeSeconds}, s-maxage=${maxAgeSeconds}`;
  } else {
    headers["Cache-Control"] = "no-cache, no-store, must-revalidate";
  }
  return new Response(JSON.stringify(data), { headers });
}

async function handleUsStockQuotes(symbols) {
  if (symbols.length === 0) return { quotes: [] };
  const query = encodeURIComponent(symbols.join(","));
  const url = `https://query1.finance.yahoo.com/v7/finance/quote?symbols=${query}`;

  const res = await fetch(url, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok) throw new Error(`Yahoo Finance returned HTTP ${res.status}`);

  const json = await res.json();
  const rawList = json?.quoteResponse?.result || [];
  const quotes = rawList.map((q) => {
    const symbol = q.symbol || "";
    const name = q.shortName || q.longName || symbol;
    const price = q.regularMarketPrice ?? 0;
    const prevClose = q.regularMarketPreviousClose ?? price;
    const open = q.regularMarketOpen ?? price;
    const high = q.regularMarketDayHigh ?? price;
    const low = q.regularMarketDayLow ?? price;
    const volume = q.regularMarketVolume ?? 0;
    const marketCap = q.marketCap ?? 0;
    const peRatio = q.trailingPE ?? null;
    const change = price - prevClose;
    const changePct = prevClose > 0 ? (change / prevClose) * 100 : 0;

    return {
      symbol,
      name,
      price,
      prev_close: prevClose,
      open,
      high,
      low,
      volume,
      market_cap: marketCap,
      pe_ratio: peRatio,
      change,
      change_pct: changePct,
      currency: q.currency || "USD",
    };
  });

  return { quotes, updated_at: Math.floor(Date.now() / 1000) };
}

async function handleUsStockSearch(query) {
  const q = (query || "").trim();
  if (!q) return { results: [] };

  const url = `https://query1.finance.yahoo.com/v1/finance/search?q=${encodeURIComponent(
    q
  )}&quotesCount=15&newsCount=0&enableFuzzyQuery=false&quotesQueryId=tss_match_phrase_query`;

  const res = await fetch(url, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok) return { results: [] };

  const json = await res.json();
  const raw = json?.quotes || [];
  const results = raw
    .filter((it) => it.quoteType === "EQUITY" || it.quoteType === "ETF")
    .map((it) => ({
      symbol: it.symbol,
      name: it.shortname || it.longname || it.symbol,
      exchange: it.exchDisp || it.exchange || "US",
      type: it.quoteType || "EQUITY",
    }));

  return { query: q, results };
}

async function handleUsStockHistory(symbol, range) {
  let interval = "1d";
  let yahooRange = "1y";

  if (range === "1d") {
    yahooRange = "1d";
    interval = "5m";
  } else if (range === "1m") {
    yahooRange = "1mo";
    interval = "1d";
  } else if (range === "6m") {
    yahooRange = "6mo";
    interval = "1d";
  } else {
    yahooRange = "1y";
    interval = "1d";
  }

  const url = `https://query1.finance.yahoo.com/v8/finance/chart/${encodeURIComponent(
    symbol
  )}?range=${yahooRange}&interval=${interval}`;

  const res = await fetch(url, { headers: { "User-Agent": USER_AGENT } });
  if (!res.ok) throw new Error(`Yahoo chart returned HTTP ${res.status}`);

  const json = await res.json();
  const result = json?.chart?.result?.[0];
  if (!result) return { symbol, range, points: [] };

  const timestamps = result.timestamp || [];
  const quote = result.indicators?.quote?.[0] || {};
  const opens = quote.open || [];
  const highs = quote.high || [];
  const lows = quote.low || [];
  const closes = quote.close || [];
  const volumes = quote.volume || [];

  const points = [];
  for (let i = 0; i < timestamps.length; i++) {
    const c = closes[i];
    if (c !== null && c !== undefined && !isNaN(c)) {
      points.push({
        time: timestamps[i],
        open: opens[i] ?? c,
        high: highs[i] ?? c,
        low: lows[i] ?? c,
        close: c,
        volume: volumes[i] ?? 0,
      });
    }
  }

  return {
    symbol,
    range,
    currency: result.meta?.currency || "USD",
    points,
    updated_at: Math.floor(Date.now() / 1000),
  };
}

// BEGIN SHARED FOREX
// Shared forex implementation, embedded into standalone Workers by sync-forex.mjs.
// All work is request-driven: no timers, dependencies, or background polling.
const marketMemory = new Map();
const marketPending = new Map();
const positive = value => typeof value === "number" && Number.isFinite(value) && value > 0;
const METAL_FACTORS = { GOLD: 1.035, SILVER: 1.05, PLATINUM: 1, PALLADIUM: 1 };
const COMMODITY_TICKERS = {
  GOLD: "GC=F", SILVER: "SI=F", OIL: "CL=F", BRENT: "BZ=F",
  PLATINUM: "PL=F", PALLADIUM: "PA=F", NATURAL_GAS: "NG=F", COPPER: "HG=F",
  WHEAT: "ZW=F", CORN: "ZC=F", SOYBEANS: "ZS=F", COFFEE: "KC=F",
  SUGAR: "SB=F", COCOA: "CC=F", COTTON: "CT=F",
};
function marketSymbol(raw) {
  const symbol = raw.trim().toUpperCase();
  return ({ CPO: "PALM_OIL", EMAS: "GOLD", PERAK: "SILVER", WTI: "OIL" })[symbol] || symbol;
}
function marketTicker(symbol) {
  return COMMODITY_TICKERS[symbol] || (/^[A-Z]{3}\/[A-Z]{3}$/.test(symbol) ? symbol.replace("/", "") + "=X" : null);
}
function marketScale(symbol) {
  if (symbol === "VND/IDR") return 1000;
  if (/^(JPY|KRW)\/IDR$/.test(symbol)) return 100;
  return ["WHEAT", "CORN", "SOYBEANS", "COFFEE", "SUGAR", "COTTON"].includes(symbol) ? 0.01 : 1;
}

// Cache provider results rather than whole watchlists, so clients share the same
// quote. 120s matches the desktop refresh; daily currency data needs only 1h.
// Keep last-known data for at most 24h and preserve its original timestamp.
async function marketData(key, url, ttl, parse, readBody = response => response.json()) {
  if (marketPending.has(key)) return marketPending.get(key);
  const pending = (async () => {
    const cache = globalThis.caches?.default;
    const cacheKey = new Request("https://api.dollarkilat.xyz/__market_cache/v2/" + encodeURIComponent(key));
    let entry = marketMemory.get(key);
    if (!entry && cache) {
      try { entry = await (await cache.match(cacheKey))?.json(); } catch (_) {}
    }
    const now = Date.now();
    const usable = entry?.value && entry.expires > now;
    if (entry?.fresh > now) return usable ? entry.value : null;
    if (entry?.retry > now) return usable ? { ...entry.value, stale: true } : null;
    let retry = now + 60000;
    try {
      const response = await fetch(url, {
        headers: { "User-Agent": USER_AGENT }, signal: AbortSignal.timeout(2000),
      });
      if (!response.ok) {
        if (response.status === 429) {
          const header = response.headers.get("Retry-After");
          const seconds = header && /^\d+$/.test(header) ? Number(header) : (Date.parse(header) - now) / 1000;
          retry = now + Math.max(60000, Number.isFinite(seconds) ? seconds * 1000 : 1200000);
        }
        throw new Error("Upstream HTTP " + response.status);
      }
      const value = parse(await readBody(response));
      if (!value) throw new Error("Invalid upstream payload");
      entry = { value, fresh: now + ttl * 1000, expires: now + 86400000, retry: 0 };
    } catch (_) {
      entry = { value: usable ? entry.value : null, fresh: 0, expires: usable ? entry.expires : now + 86400000, retry };
    }
    if (marketMemory.size >= 128) marketMemory.delete(marketMemory.keys().next().value);
    marketMemory.set(key, entry);
    if (cache) {
      try {
        await cache.put(cacheKey, Response.json(entry, { headers: { "Cache-Control": "public, max-age=86400" } }));
      } catch (_) {} // An unavailable edge cache must not discard valid prices.
    }
    return entry.value ? { ...entry.value, stale: entry.retry > now } : null;
  })();
  marketPending.set(key, pending);
  try { return await pending; } finally { marketPending.delete(key); }
}

async function marketChart(ticker, history = false) {
  const query = history ? "range=1y&interval=1d" : "range=1d&interval=30m";
  return marketData(`chart:${ticker}:${history}`, `https://query1.finance.yahoo.com/v8/finance/chart/${encodeURIComponent(ticker)}?${query}`, history ? 600 : 120, json => {
    const result = json?.chart?.result?.[0];
    const meta = result?.meta;
    if (!positive(meta?.regularMarketPrice) || !positive(meta?.regularMarketTime)) return null;
    const times = result.timestamp || [];
    const closes = result.indicators?.quote?.[0]?.close || [];
    const points = times.flatMap((time, i) => positive(time) && positive(closes[i]) ? [{ time, close: closes[i] }] : []);
    if (history && !points.length) return null;
    const price = meta.regularMarketPrice;
    return { price, prev: positive(meta.chartPreviousClose) ? meta.chartPreviousClose : price,
      open: positive(meta.regularMarketOpen) ? meta.regularMarketOpen : price,
      low: positive(meta.regularMarketDayLow) ? meta.regularMarketDayLow : price,
      high: positive(meta.regularMarketDayHigh) ? meta.regularMarketDayHigh : price,
      updated_at: meta.regularMarketTime, points, source: "yahoo" };
  });
}

async function marketCurrencies() {
  for (const url of [
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json",
    "https://latest.currency-api.pages.dev/v1/currencies/usd.json",
  ]) {
    const data = await marketData(url, url, 3600, json => {
      const timestamp = Date.parse(json?.date + "T00:00:00Z") / 1000;
      return positive(json?.usd?.idr) && positive(timestamp)
        ? { rates: json.usd, updated_at: timestamp, source: "currency-api (daily)" } : null;
    });
    if (data) return data;
  }
  return null;
}

async function marketPalmOil() {
  const url = "https://bepi.mpob.gov.my/admin2/price_local_daily_view_cpo_msia.php?jenis=1W&more=Y";
  return marketData("mpob:cpo:local-delivered:v1", url, 3600, html => {
    const text = html.replace(/<[^>]*>/g, " ").replace(/&nbsp;|&#160;/gi, " ");
    const priceText = text.match(/(?:MYR|RM)\s*([\d,]+(?:\.\d{1,2})?)/i)?.[1];
    const dateText = text.match(/Latest\s+Price:\s*([\d]{1,2}\s+[A-Za-z]+\s+[\d]{4})/i)?.[1];
    if (!priceText || !dateText) return null;
    const price = Number(priceText.replaceAll(",", ""));
    // The heading provides a date but not whether this is the morning or
    // afternoon revision. Anchor to local midnight to avoid future timestamps.
    const updatedAt = Date.parse(`${dateText} 00:00:00 GMT+0800`) / 1000;
    const now = Math.floor(Date.now() / 1000);
    if (!positive(price) || price < 1000 || price > 20000 || !positive(updatedAt)
      || updatedAt > now + 86400 || now - updatedAt > 7 * 86400) return null;
    // MPOB reports a weighted average local-delivered CPO price in MYR/tonne.
    // It is a daily physical price, not the unavailable FCPO futures ticker.
    return { price, prev: price, open: price, low: price, high: price, points: [],
      updated_at: updatedAt, source: "MPOB (daily local-delivered CPO)" };
  }, response => response.text());
}

async function handleForexBoard(requested) {
  const symbols = [...new Set(requested.map(marketSymbol))].slice(0, 20);
  const tickers = new Set(symbols.map(marketTicker).filter(Boolean));
  if (symbols.some(symbol => symbol in METAL_FACTORS)) tickers.add("USDIDR=X");
  const results = new Map(await Promise.all([...tickers].map(async ticker => [ticker, await marketChart(ticker)])));
  const needsDaily = symbols.some(symbol => symbol !== "PALM_OIL" && !results.get(marketTicker(symbol)))
    || (symbols.some(s => s in METAL_FACTORS) && !results.get("USDIDR=X"));
  const [daily, palmOil] = await Promise.all([
    needsDaily ? marketCurrencies() : Promise.resolve(null),
    symbols.includes("PALM_OIL") ? marketPalmOil() : Promise.resolve(null),
  ]);
  const idrQuote = results.get("USDIDR=X");
  const idr = idrQuote?.price || daily?.rates.idr;
  const quotes = [];
  for (const symbol of symbols) {
    let data = symbol === "PALM_OIL" ? palmOil : results.get(marketTicker(symbol));
    if (!data && daily) {
      const [base, quote] = symbol.toLowerCase().split("/");
      const a = base === "usd" ? 1 : daily.rates[base];
      const b = quote === "usd" ? 1 : daily.rates[quote];
      let price = positive(a) && positive(b) ? b / a : null;
      const metal = ({ GOLD: "xau", SILVER: "xag", PLATINUM: "xpt", PALLADIUM: "xpd" })[symbol];
      if (metal && positive(daily.rates[metal])) price = 1 / daily.rates[metal];
      if (positive(price)) data = { price, prev: price, open: price, low: price, high: price, points: [], ...daily };
    }
    if (!data) continue; // Missing data stays missing; never manufacture a price.
    let factor = marketScale(symbol);
    let updated = data.updated_at;
    let stale = !!data.stale;
    if (symbol in METAL_FACTORS) {
      if (!positive(idr)) continue;
      factor = idr / 31.1034768 * METAL_FACTORS[symbol];
      updated = Math.min(updated, idrQuote?.updated_at || daily.updated_at);
      stale ||= !!(idrQuote?.stale || daily?.stale);
    }
    const catalog = FOREX_CATALOG.find(item => item.code === symbol)
      || (symbol === "PALM_OIL"
        ? { name: "Crude Palm Oil (local delivered)", unit: "per tonne (MYR; daily)" }
        : null);
    const rate = data.price * factor;
    const prev = data.prev * factor;
    quotes.push({ symbol, code: symbol, name: catalog?.name || symbol, rate,
      open: data.open * factor, prev_close: prev, prev_rate: prev,
      low: data.low * factor, high: data.high * factor,
      change_pct: (rate / prev - 1) * 100, is_gold: symbol === "GOLD",
      unit: symbol in METAL_FACTORS ? "per 1 gram (IDR)" : catalog?.unit || "per unit",
      spark: data.points.map((p, i) => [i * 30, p.close * factor]),
      updated_at: updated, source: data.source, stale });
  }
  return { quotes, updated_at: quotes.length ? Math.min(...quotes.map(q => q.updated_at)) : 0,
    unavailable: symbols.filter(symbol => !quotes.some(q => q.symbol === symbol)) };
}

async function handleForexHistory(raw) {
  const symbol = marketSymbol(raw);
  // Accept legacy Yahoo ticker requests as well as the desktop's canonical names.
  const ticker = marketTicker(symbol) || (/^[A-Z0-9]+=(X|F)$/.test(symbol) ? symbol : null);
  const empty = { symbol, points: [], closes: [], updated_at: 0 };
  if (!ticker) return empty;
  const data = await marketChart(ticker, true);
  if (!data) return empty;
  let factor = marketScale(symbol);
  let updatedAt = data.updated_at;
  let stale = !!data.stale;
  if (symbol in METAL_FACTORS) {
    const idr = await marketChart("USDIDR=X");
    const daily = idr ? null : await marketCurrencies();
    const rate = idr?.price || daily?.rates.idr;
    if (!positive(rate)) return empty;
    updatedAt = Math.min(updatedAt, idr?.updated_at || daily.updated_at);
    stale ||= !!(idr?.stale || daily?.stale);
    // Match the desktop: historical USD spot converted at the current IDR rate.
    factor = rate / 31.1034768 * METAL_FACTORS[symbol];
  }
  const points = data.points.map(p => ({ time: p.time, close: p.close * factor }));
  return { symbol, yahoo_symbol: ticker, points,
    closes: points.map(p => [Math.floor(p.time / 86400), p.close]),
    updated_at: updatedAt, stale };
}
// END SHARED FOREX

function handleForexSearch(query) {
  const q = (query || "").trim().toUpperCase();
  if (!q) {
    return { results: FOREX_CATALOG.slice(0, 15) };
  }
  const filtered = FOREX_CATALOG.filter(
    (item) => item.code.includes(q) || item.name.toUpperCase().includes(q)
  ).slice(0, 20);

  return { query, results: filtered };
}

async function handleCryptoBoard(symbols) {
  if (symbols.length === 0) return { quotes: [] };
  const pairMap = {};
  symbols.forEach((s) => {
    pairMap[`${s}USDT`] = s;
  });

  const res = await fetch("https://api.binance.com/api/v3/ticker/24hr");
  if (!res.ok) throw new Error(`Binance API returned HTTP ${res.status}`);

  const allTickers = await res.json();
  const quotes = allTickers
    .filter((t) => pairMap[t.symbol])
    .map((t) => {
      const code = pairMap[t.symbol];
      const price = parseFloat(t.lastPrice) || 0;
      const prevClose = parseFloat(t.prevClosePrice) || price;
      const high = parseFloat(t.highPrice) || price;
      const low = parseFloat(t.lowPrice) || price;
      const volume = parseFloat(t.volume) || 0;
      const changePct = parseFloat(t.priceChangePercent) || 0;

      return {
        code,
        price,
        prev_close: prevClose,
        high,
        low,
        volume,
        change_pct: changePct,
      };
    })
    .filter(Boolean);

  return { quotes, updated_at: Math.floor(Date.now() / 1000) };
}

async function handleCryptoSearch(query) {
  const q = (query || "").trim().toUpperCase();
  const res = await fetch("https://api.binance.com/api/v3/ticker/price");
  if (!res.ok) return { results: [] };
  const all = await res.json();
  const usdtPairs = all
    .filter((p) => p.symbol.endsWith("USDT"))
    .map((p) => p.symbol.replace("USDT", ""))
    .filter((c) => !q || c.includes(q))
    .slice(0, 15);

  return { query: q, results: usdtPairs };
}
