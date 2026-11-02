import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import assert from 'node:assert/strict';

const files = ['worker.js', '_worker.js', 'deploy-pages/_worker.js', 'workers/dollarkilat-api/worker.js', 'functions/[[path]].js'];
function load(file, fetcher) {
  const source = readFileSync(new URL('../../' + file, import.meta.url), 'utf8')
    .replace('export default', 'const worker =').replace('export async function', 'async function');
  const box = vm.createContext({ fetch: fetcher, Response, Request, URL, AbortSignal, console });
  vm.runInContext(source, box);
  return expression => vm.runInContext(expression, box);
}
const stamp = 1793577600;
function chart(price = 80) {
  return { chart: { result: [{ meta: { regularMarketPrice: price, chartPreviousClose: 75, regularMarketTime: stamp }, timestamp: [stamp - 86400, stamp], indicators: { quote: [{ close: [75, price] }] } }] } };
}
for (const file of files) {
  test(file + ': live quotes match desktop schema and never invent unavailable prices', async () => {
    const run = load(file, async url => {
      if (url.includes('currency')) return Response.json({ date: '2026-11-02', usd: { usd: 1, idr: 16000, eur: 0.8, jpy: 150 } });
      return Response.json(chart(url.includes('USDIDR') ? 16000 : 80));
    });
    const board = await run('handleForexBoard(["OIL", "JPY/IDR", "GOLD", "UNKNOWN"])');
    const oil = board.quotes.find(q => q.symbol === 'OIL');
    assert.ok(oil, 'desktop must receive symbol rather than only code');
    assert.equal(oil.rate, 80);
    for (const key of ['name', 'open', 'prev_close', 'low', 'high', 'change_pct', 'is_gold', 'unit', 'spark']) assert.ok(key in oil, key);
    assert.equal(board.quotes.some(q => q.symbol === 'UNKNOWN'), false);
    const history = await run('handleForexHistory("JPY/IDR")');
    assert.equal(history.closes[1][1], 8000);
    assert.equal(history.closes[1][0], Math.floor(stamp / 86400));
  });
  test(file + ': rate limits do not fabricate a successful board or hammer upstream', async () => {
    let calls = 0;
    const run = load(file, async () => { calls++; return new Response('', { status: 429, headers: { 'Retry-After': '120' } }); });
    const first = await run('handleForexBoard(["OIL"])');
    const before = calls;
    await run('handleForexBoard(["OIL"])');
    assert.equal(first.quotes.length, 0);
    assert.equal(calls, before, 'cooldown must suppress repeated requests');
  });
  test(file + ': requests share upstream results across watchlists', async () => {
    let calls = 0;
    const run = load(file, async () => { calls++; return Response.json(chart()); });
    await Promise.all([run('handleForexBoard(["OIL"])'), run('handleForexBoard(["OIL"])')]);
    const before = calls;
    await run('handleForexBoard(["OIL"])');
    assert.equal(calls, before);
  });
  test(file + ': stale prices retain observation time and expire after 24 hours', async () => {
    let healthy = true;
    let calls = 0;
    const run = load(file, async () => {
      calls++;
      return healthy ? Response.json(chart(90)) : new Response('', { status: 429, headers: { 'Retry-After': '120' } });
    });
    run('Date.now = () => 1000000');
    const fresh = await run('handleForexBoard(["OIL"])');
    healthy = false;
    run('Date.now = () => 1121000');
    const stale = await run('handleForexBoard(["OIL"])');
    assert.equal(stale.quotes[0].rate, 90);
    assert.equal(stale.quotes[0].stale, true);
    assert.equal(stale.updated_at, fresh.updated_at);
    const before = calls;
    await run('handleForexBoard(["OIL"])');
    assert.equal(calls, before);
    run('Date.now = () => 90000000');
    const expired = await run('handleForexBoard(["OIL"])');
    assert.equal(expired.quotes.length, 0);
    healthy = true;
    run('Date.now = () => 90200000');
    const recovered = await run('handleForexBoard(["OIL"])');
    assert.equal(recovered.quotes[0].stale, false);
  });
  test(file + ': daily fallback converts cross rates without faking movement or missing commodities', async () => {
    const run = load(file, async url => url.includes('currency')
      ? Response.json({ date: '2026-11-02', usd: { usd: 1, idr: 16000, eur: 0.8, jpy: 160, xag: 0.025 } })
      : new Response('', { status: 429 }));
    const board = await run('handleForexBoard(["EUR/USD", "JPY/IDR", "SILVER", "OIL"])');
    assert.equal(board.quotes.find(q => q.symbol === 'EUR/USD').rate, 1.25);
    assert.equal(board.quotes.find(q => q.symbol === 'JPY/IDR').rate, 10000);
    assert.ok(Math.abs(board.quotes.find(q => q.symbol === 'SILVER').rate - 21605.302) < 0.01);
    assert.equal(board.quotes[0].change_pct, 0);
    assert.equal(board.quotes[0].source, 'currency-api (daily)');
    assert.deepEqual(Array.from(board.unavailable), ['OIL']);
  });
  test(file + ': VND blocks and cent-denominated futures use the displayed units', async () => {
    const run = load(file, async () => Response.json(chart(550)));
    const board = await run('handleForexBoard(["VND/IDR", "WHEAT"])');
    assert.equal(board.quotes.find(q => q.symbol === 'VND/IDR').rate, 550000);
    assert.equal(board.quotes.find(q => q.symbol === 'WHEAT').rate, 5.5);
    const history = await run('handleForexHistory("WHEAT")');
    assert.equal(history.closes[1][1], 5.5);
  });
  test(file + ': metal history preserves stale FX conversion metadata', async () => {
    let failFx = false;
    const run = load(file, async url => {
      if (url.includes('USDIDR') && failFx) return new Response('', { status: 429 });
      const data = chart(url.includes('USDIDR') ? 16000 : 80);
      data.chart.result[0].meta.regularMarketTime = url.includes('USDIDR') ? stamp - 3600 : stamp;
      return Response.json(data);
    });
    run('Date.now = () => 1000000');
    await run('handleForexBoard(["GOLD"])');
    failFx = true;
    run('Date.now = () => 1121000');
    const history = await run('handleForexHistory("GOLD")');
    assert.equal(history.stale, true);
    assert.equal(history.updated_at, stamp - 3600);
  });
}
