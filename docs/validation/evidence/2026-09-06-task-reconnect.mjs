import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const taskId = 'task:f932f3c82add0b649c3ffc5d2328e179';
const evidence = [];
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
  for (const width of [1440, 390]) {
    const context = await browser.newContext({ serviceWorkers: 'block', storageState: '/var/tmp/noema-audit-credentials/browser-session.json', viewport: { width, height: 1000 } });
    await context.addInitScript(() => {
      const Original = window.WebSocket;
      window.__auditSockets = [];
      window.WebSocket = class extends Original {
        constructor(...args) { super(...args); window.__auditSockets.push(this); }
      };
    });
    const page = await context.newPage();
    const events = [];
    let phase = 'initial';
    let acknowledge;
    const reconnected = new Promise(resolve => { acknowledge = resolve; });
    page.on('websocket', ws => {
      ws.on('framereceived', event => {
        try {
          const body = JSON.parse(event.payload.toString());
          if (body.type === 'connection_ack') {
            events.push({ phase, type: 'ack' });
            if (phase === 'reconnected') acknowledge(true);
          }
          if (body.payload?.data?.taskEvents) events.push({ phase, type: 'taskEvent' });
        } catch {}
      });
      ws.on('close', () => events.push({ phase, type: 'close' }));
    });
    page.on('response', response => {
      if (!response.url().endsWith('/graphql')) return;
      const operation = response.request().postDataJSON()?.operationName;
      if (['TasksTaskDetail', 'TasksTaskRunItems'].includes(operation)) events.push({ phase, type: 'query', operation, status: response.status() });
    });
    await page.goto(origin + '/tasks/' + encodeURIComponent(taskId) + '?terminal=all');
    await page.getByRole('button', { name: 'Transcript', exact: true }).click();
    await page.getByText('Task Planner · Planner · Waiting for approval', { exact: true }).waitFor();
    phase = 'reconnected';
    await page.evaluate(() => window.__auditSockets.forEach(socket => socket.close(4000, 'Audit connection interruption')));
    let timer;
    const acknowledged = await Promise.race([reconnected, new Promise(resolve => { timer = setTimeout(() => resolve(false), 20000); })]);
    clearTimeout(timer);
    const closed = events.some(event => event.type === 'close');
    assert.equal(closed, true);
    assert.equal(acknowledged, true);
    await page.getByText('Task Planner · Planner · Waiting for approval', { exact: true }).waitFor();
    const read = await page.evaluate(async id => {
      const response = await fetch('/graphql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query: 'query($id:String!){task(taskId:$id){taskId stage{key} runs{runId status}}}', variables: { id } }) });
      const body = await response.json();
      if (body.errors) throw new Error('Task read failed');
      return body.data.task;
    }, taskId);
    evidence.push({ width, acknowledged, closed, waitingRunHeaderPreserved: true, task: read, events });
    await context.close();
  }
} finally {
  await writeFile('docs/validation/evidence/2026-09-06-task-reconnect-results.json', JSON.stringify({ instance: origin, taskId, scope: 'Read-only waiting Task. Test closes the browser WebSocket with code 4000. Earlier offline emulation did not close that socket and is not reconnect evidence. No missed state changes or later completion were induced. Same-hostname origin access.', evidence }, null, 2) + '\n');
  await browser.close();
}
