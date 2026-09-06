import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const evidence = { instance: origin, scope: 'Emulated installed browser, notification permission, focus, and registered push subscription. Presence frames terminate in the test driver. No live registration or alert.', clients: [] };
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
  for (const width of [1440, 390]) {
    const context = await browser.newContext({ serviceWorkers: 'allow', storageState: '/var/tmp/noema-audit-credentials/browser-session.json', viewport: { width, height: 1000 } });
    await context.addInitScript(() => {
      Object.defineProperty(navigator, 'standalone', { get: () => true });
      Object.defineProperty(Notification, 'permission', { get: () => 'granted' });
      document.hasFocus = () => true;
      PushManager.prototype.getSubscription = async () => ({ endpoint: 'https://example.com/audit-push' });
    });
    await context.route('**/graphql', async route => {
      const body = route.request().postDataJSON();
      if (body.operationName === 'WebPushStatus') return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ data: { webPushStatus: { available: true, blocker: null, applicationServerKey: null, subscriptionId: 'audit-surface-presence' } } }) });
      if (/\bmutation\b/.test(body.query || '')) throw new Error('Unexpected mutation');
      await route.continue();
    });
    const record = { width, events: [] }, active = new Set();
    evidence.clients.push(record);
    let phase = 'Chat';
    await context.routeWebSocket('**/graphql/ws', ws => {
      const server = ws.connectToServer();
      ws.onMessage(message => {
        const body = JSON.parse(String(message));
        if (body.type === 'subscribe' && /\bWebPushPresence\b/.test(body.payload?.query || '')) {
          active.add(body.id); record.events.push({ phase, type: 'subscribe' });
          ws.send(JSON.stringify({ id: body.id, type: 'next', payload: { data: { webPushPresence: { subscriptionId: 'audit-surface-presence', ready: true } } } }));
        } else if (body.type === 'complete' && active.has(body.id)) {
          active.delete(body.id); record.events.push({ phase, type: 'complete' });
        } else server.send(message);
      });
    });
    const page = await context.newPage();
    page.on('pageerror', error => record.events.push({ type: 'pageerror', message: error.message }));
    await page.goto(origin);
    await page.evaluate(async () => { await navigator.serviceWorker.register('/assets/sw.js', { scope: '/' }); await navigator.serviceWorker.ready; });
    await page.reload();
    await page.getByRole('textbox', { name: 'Message', exact: true }).waitFor();
    const waitPresence = async count => {
      const deadline = Date.now() + 10000;
      while (active.size !== count && Date.now() < deadline) await page.waitForTimeout(100);
      assert.equal(active.size, count);
    };
    await waitPresence(1);
    for (const [surface, href] of [['Tasks', '/tasks'], ['Settings', '/settings'], ['Chat', '/']]) {
      phase = surface;
      await page.locator(surface === 'Settings' ? 'a[href^="/settings/"]' : `a[href="${href}"]`).first().click();
      await waitPresence(surface === 'Chat' ? 1 : 0);
      record.events.push({ phase, type: 'verified', active: active.size });
    }
    await context.close();
  }
} catch (error) { evidence.error = String(error); throw error; }
finally { await writeFile('docs/validation/evidence/2026-09-06-notification-surface-presence-results.json', JSON.stringify(evidence, null, 2) + '\n'); await browser.close(); }
