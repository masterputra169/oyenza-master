import { readFileSync, writeFileSync } from 'node:fs';
const root = new URL('../../', import.meta.url);
const shared = readFileSync(new URL('forex-source.js', import.meta.url), 'utf8');
const begin = '// BEGIN SHARED FOREX\n';
const end = '// END SHARED FOREX\n';
for (const file of ['worker.js', '_worker.js', 'deploy-pages/_worker.js', 'workers/dollarkilat-api/worker.js', 'functions/[[path]].js']) {
  const url = new URL(file, root);
  const old = readFileSync(url, 'utf8').replaceAll('\r\n', '\n');
  let next;
  if (old.includes(begin)) {
    next = old.slice(0, old.indexOf(begin)) + begin + shared + end + old.slice(old.indexOf(end) + end.length);
  } else {
    const start = old.indexOf('async function handleForexBoard(');
    const search = old.indexOf('function handleForexSearch(', start);
    const hist = old.indexOf('async function handleForexHistory(', search);
    const after = old.indexOf('async function handleCryptoBoard(', hist);
    if ([start, search, hist, after].some(i => i < 0)) throw new Error('Cannot locate forex handlers: ' + file);
    next = old.slice(0, start) + begin + shared + end + '\n' + old.slice(search, hist) + old.slice(after);
  }
  if (process.argv.includes('--check')) {
    if (old !== next) throw new Error('Run node workers/dollarkilat-api/sync-forex.mjs: ' + file);
  } else writeFileSync(url, next);
}
