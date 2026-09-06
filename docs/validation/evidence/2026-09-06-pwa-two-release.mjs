import https from 'node:https';
import { readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import assert from 'node:assert/strict';

const origin = 'https://noema.kevinpei.com:44443';
const releaseA = '/var/tmp/noema-pwa-release-a';
const releaseB = '/var/tmp/noema-pwa-release-b';
const cert = {
  key: await readFile('/var/tmp/noema-pwa-fixture.key'),
  cert: await readFile('/var/tmp/noema-pwa-fixture.crt')
};
let release = 'a';
const served = [];
const proxied = [];
const server = https.createServer(cert, async (req, res) => {
  const requestPath = new URL(req.url ?? '/', origin).pathname;
  if (requestPath === '/__switch') {
    release = 'b';
    res.writeHead(200, {'content-type': 'text/plain', 'cache-control': 'no-store'});
    res.end('release-b');
    return;
  }
  if (requestPath.startsWith('/assets/')) {
    const name = decodeURIComponent(requestPath.slice('/assets/'.length));
    if (!name || name.includes('/') || name.includes('\\') || name === '.' || name === '..') {
      res.writeHead(404); res.end(); return;
    }
    const filePath = join(release === 'a' ? releaseA : releaseB, name);
    try {
      const info = await stat(filePath);
      if (!info.isFile()) throw new Error('not a file');
      served.push({release, name});
      const body = await readFile(filePath);
      const contentType = name.endsWith('.js') ? 'text/javascript; charset=utf-8'
        : name.endsWith('.css') ? 'text/css; charset=utf-8'
        : name.endsWith('.html') ? 'text/html; charset=utf-8'
        : name.endsWith('.webmanifest') ? 'application/manifest+json'
        : name.endsWith('.json') ? 'application/json'
        : 'application/octet-stream';
      res.writeHead(200, {
        'content-type': contentType,
        'content-length': body.length,
        'cache-control': name === 'sw.js' ? 'no-store' : 'public, max-age=31536000, immutable',
        ...(name === 'sw.js' ? {'service-worker-allowed': '/'} : {})
      });
      res.end(body);
      return;
    } catch {
      res.writeHead(404); res.end(); return;
    }
  }
  if (requestPath === '/' || requestPath === '/index.html') {
    const body = await readFile(join(release === 'a' ? releaseA : releaseB, 'index.html'));
    res.writeHead(200, {'content-type': 'text/html; charset=utf-8', 'content-length': body.length, 'cache-control': 'no-store'});
    res.end(body);
    return;
  }
  const headers = {...req.headers, host: 'noema.kevinpei.com'};
  if (headers.origin) headers.origin = 'https://noema.kevinpei.com';
  if (headers.referer) headers.referer = 'https://noema.kevinpei.com/';
  delete headers.connection;
  const upstream = https.request({
    hostname: '127.0.0.1',
    port: 443,
    servername: 'noema.kevinpei.com',
    path: req.url,
    method: req.method,
    headers,
    rejectUnauthorized: false
  }, upstreamResponse => {
    proxied.push({path: requestPath, status: upstreamResponse.statusCode ?? 0});
    const responseHeaders = {...upstreamResponse.headers};
    delete responseHeaders.connection;
    res.writeHead(upstreamResponse.statusCode ?? 502, responseHeaders);
    upstreamResponse.pipe(res);
  });
  upstream.on('error', error => {
    if (!res.headersSent) res.writeHead(502, {'content-type': 'text/plain'});
    res.end(String(error));
  });
  req.pipe(upstream);
});
await new Promise(resolve => server.listen(44443, '127.0.0.1', resolve));

const browser = await chromium.launch({
  headless: true,
  args: [
    '--no-sandbox',
    '--ignore-certificate-errors',
    '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'
  ]
});
const context = await browser.newContext({
  serviceWorkers: 'allow',
  storageState: '/var/tmp/noema-audit-credentials/browser-session.json',
  viewport: {width: 390, height: 1000}
});
await context.addInitScript(() => {
  Object.defineProperty(navigator, 'standalone', {get: () => true});
});
const page = await context.newPage();
page.setDefaultTimeout(30000);
const errors = [];
page.on('pageerror', error => errors.push(error.message));
page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
let reloads = 0;
page.on('framenavigated', frame => { if (frame === page.mainFrame()) reloads += 1; });
const results = [];
try {
  await page.goto(origin + '/');
  await page.getByRole('textbox', {name: 'Message', exact: true}).waitFor();
  await page.waitForFunction(async () => {
    const r = await navigator.serviceWorker.getRegistration('/');
    return r?.active?.state === 'activated' && !!navigator.serviceWorker.controller;
  });
  results.push({check: 'Release A worker active', passed: true});
  const before = await page.evaluate(async () => {
    const r = await navigator.serviceWorker.getRegistration('/');
    return {active: r?.active?.scriptURL ?? null, caches: await caches.keys(), controller: navigator.serviceWorker.controller?.scriptURL ?? null};
  });
  results.push({check: 'Release A cache', before});
  const draft = 'PWA release transition draft café 日本語 🧭';
  const input = page.getByRole('textbox', {name: 'Message', exact: true});
  await input.fill(draft);
  await page.waitForFunction(async expected => new Promise(resolve => {
    const request = indexedDB.open('noema-pwa');
    request.onsuccess = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains('records')) { db.close(); resolve(false); return; }
      const read = db.transaction('records').objectStore('records').getAll();
      read.onsuccess = () => { db.close(); resolve(read.result.some(row => row.key.startsWith('chat-draft:') && row.value === expected)); };
      read.onerror = () => { db.close(); resolve(false); };
    };
    request.onerror = () => resolve(false);
  }), draft);
  results.push({check: 'Draft saved before release switch', passed: true});
  const switched = await new Promise((resolve, reject) => {
    const request = https.request({hostname: '127.0.0.1', port: 44443, path: '/__switch', rejectUnauthorized: false}, response => {
      let body = ''; response.setEncoding('utf8'); response.on('data', chunk => body += chunk); response.on('end', () => resolve(body));
    });
    request.on('error', reject); request.end();
  });
  assert.equal(switched, 'release-b');
  const prepared = await page.evaluate(async () => {
    const registration = await navigator.serviceWorker.getRegistration('/');
    if (!registration) throw new Error('missing registration');
    await registration.update();
    const deadline = Date.now() + 30000;
    while (!registration.waiting && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 100));
    return {waiting: registration.waiting?.state ?? null, installing: registration.installing?.state ?? null, caches: await caches.keys()};
  });
  results.push({check: 'Release B installed before safe activation', prepared});
  await page.evaluate(() => window.dispatchEvent(new Event('online')));
  await page.evaluate(async () => {
    const deadline = Date.now() + 90000;
    const registration = await navigator.serviceWorker.getRegistration('/');
    if (!registration) throw new Error('missing registration');
    while (!registration.waiting && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 100));
    if (!registration.waiting) throw new Error('release B never reached waiting state');
    while (registration.waiting && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 100));
    if (registration.waiting) throw new Error('release B did not activate');
  });
  const restoredInput = page.getByRole('textbox', {name: 'Message', exact: true});
  await restoredInput.waitFor({timeout: 30000});
  assert.equal(await restoredInput.inputValue(), draft);
  const restoredCaches = await page.evaluate(() => caches.keys());
  assert.ok(restoredCaches.some(name => name.includes('noema-pwa-b')), 'release B cache missing after activation');
  const after = await page.evaluate(async () => {
    const r = await navigator.serviceWorker.getRegistration('/');
    return {active: r?.active?.scriptURL ?? null, waiting: r?.waiting?.scriptURL ?? null, caches: await caches.keys(), controller: navigator.serviceWorker.controller?.scriptURL ?? null};
  });
  results.push({check: 'Release B update activates and preserves draft', passed: true, after, reloads});
} catch (error) {
  results.push({check: 'Execution stopped', passed: false, error: error instanceof Error ? error.message : String(error), reloads});
} finally {
  await browser.close();
  await new Promise(resolve => server.close(resolve));
}
const releaseCounts = served.reduce((counts, item) => {
  counts[item.release] = (counts[item.release] ?? 0) + 1;
  return counts;
}, {});
const output = {instance: origin, setup: 'Local HTTPS fixture serves release A, switches to generated release B, and proxies authenticated API requests to noema.kevinpei.com.', results, releaseCounts, proxied: proxied.slice(-20), errors};
await writeFile('/var/tmp/noema-pwa-two-release-results.json', JSON.stringify(output, null, 2) + '\n');
console.log(JSON.stringify(output, null, 2));
