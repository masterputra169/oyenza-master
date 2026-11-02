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
