import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const options = { serviceWorkers: 'block', storageState: '/var/tmp/noema-audit-credentials/browser-session.json' };
const evidence = { instance: origin, clients: [], scope: 'Live synthetic Task; same-hostname origin access.' };
const persist = () => writeFile('docs/validation/evidence/2026-09-06-task-reconnect-completion-results.json', JSON.stringify(evidence, null, 2) + '\n');
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
  const control = await browser.newContext(options), page = await control.newPage();
  await page.goto(origin);
  const gql = (query, variables = {}) => page.evaluate(async ({ query, variables }) => {
    const body = await (await fetch('/graphql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query, variables }) })).json();
    if (body.errors) throw new Error(JSON.stringify(body.errors));
    return body.data;
  }, { query, variables });
  const created = await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId revision generation}}}', { input: { workspaceId: 'workspace:personal', title: 'Migration audit — Task connection gap', taskDocument: '# Connection recovery audit\n\nCalculate 17 × 19. Write exactly 323 to RESULT.md. Use no external services. Change no files outside this Task.', clientMutationId: crypto.randomUUID() } });
  const task = created.captureTask.task;
  evidence.taskId = task.taskId;
  await persist();
  const viewers = [];
  for (const width of [1440, 390]) {
    const context = await browser.newContext({ ...options, viewport: { width, height: 1000 } });
    await context.addInitScript(() => {
      const Original = window.WebSocket;
      window.__auditSockets = [];
      window.WebSocket = class extends Original { constructor(...args) { super(...args); window.__auditSockets.push(this); } };
    });
    const view = await context.newPage(), record = { width, events: [] };
    evidence.clients.push(record);
    let phase = 'initial';
    view.on('websocket', ws => {
      ws.on('close', () => record.events.push({ phase, type: 'close' }));
      ws.on('framereceived', event => {
        try {
          const body = JSON.parse(event.payload.toString());
          if (body.type === 'connection_ack') record.events.push({ phase, type: 'ack' });
          if (body.payload?.data?.taskEvents) record.events.push({ phase, type: 'taskEvent', kind: body.payload.data.taskEvents.kind });
        } catch {}
      });
    });
    view.on('response', response => {
      if (!response.url().endsWith('/graphql')) return;
      const operation = response.request().postDataJSON()?.operationName;
      if (['TasksTaskDetail', 'TasksTaskRunItems'].includes(operation)) record.events.push({ phase, type: 'query', operation, status: response.status() });
    });
    await view.goto(origin + '/tasks/' + encodeURIComponent(task.taskId) + '?terminal=all');
    await view.getByRole('button', { name: 'Transcript', exact: true }).click();
    viewers.push({ context, page: view, record, setPhase(value) { phase = value; } });
  }
  const waitState = wanted => page.evaluate(({ taskId, wanted }) => new Promise((resolve, reject) => {
    const ws = new WebSocket('wss://' + location.host + '/graphql/ws', 'graphql-transport-ws');
    const timer = setTimeout(() => { ws.close(); reject(new Error('Task state timeout: ' + wanted)); }, 240000);
    let done = false;
    const check = async () => {
      if (done) return;
      const body = await (await fetch('/graphql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query: 'query($id:String!){task(taskId:$id){taskId revision generation stage{key} resultDocument runs{runId kind status}}}', variables: { id: taskId } }) })).json();
      if (body.errors) { done = true; clearTimeout(timer); ws.close(); reject(new Error('Observer Task read failed')); return; }
      const task = body.data.task;
      const terminal = ['done', 'failed', 'cancelled', 'waiting'].includes(task.stage.key);
      const matched = wanted === 'active' ? task.runs.some(run => run.status === 'RUNNING') : wanted === 'changed' ? task.runs.some(run => run.status === 'COMPLETED') : terminal;
      if (matched || terminal) { done = true; clearTimeout(timer); ws.close(); resolve(task); }
    };
    ws.onopen = () => ws.send(JSON.stringify({ type: 'connection_init' }));
    ws.onmessage = event => {
      const body = JSON.parse(event.data);
      if (body.type === 'connection_ack') { ws.send(JSON.stringify({ id: 'audit', type: 'subscribe', payload: { query: 'subscription($id:String!){taskEvents(taskId:$id){kind}}', variables: { id: taskId } } })); void check(); }
      else if (body.type === 'next') void check();
    };
  }), { taskId: task.taskId, wanted });
  await gql('mutation($input:QueueTaskInput!){queueTask(input:$input){task{taskId}}}', { input: { taskId: task.taskId, expectedRevision: task.revision, expectedGeneration: task.generation, clientMutationId: crypto.randomUUID() } });
  evidence.beforeGap = await waitState('active');
  assert.ok(evidence.beforeGap.runs.some(run => run.status === 'RUNNING'));
  for (const viewer of viewers) {
    await viewer.page.getByText(/Task Planner · Planner · Running/).waitFor();
    viewer.setPhase('offline');
    await viewer.context.setOffline(true);
    await viewer.page.evaluate(() => window.__auditSockets.forEach(socket => socket.close(4000, 'Audit connection gap')));
  }
  evidence.duringGap = await waitState('changed');
  await persist();
  for (const viewer of viewers) { viewer.setPhase('reconnected'); await viewer.context.setOffline(false); }
  evidence.completed = await waitState('terminal');
  assert.equal(evidence.completed.stage.key, 'done');
  assert.equal(evidence.completed.resultDocument.trim(), '323');
  for (const viewer of viewers) {
    await viewer.page.getByText('Task Reviewer · Reviewer · Completed', { exact: true }).waitFor({ timeout: 30000 });
    viewer.record.completedHeaderVisible = true;
    await viewer.page.getByRole('button', { name: 'Workspace', exact: true }).click();
    await viewer.page.getByRole('button', { name: 'Result', exact: true }).click();
    await viewer.page.getByText('323', { exact: true }).first().waitFor();
    viewer.record.exactResultVisible = true;
    assert.ok(viewer.record.events.some(event => event.phase === 'offline' && event.type === 'close'));
    assert.ok(viewer.record.events.some(event => event.phase === 'reconnected' && event.type === 'ack'));
  }
} catch (error) { evidence.error = String(error); throw error; }
finally { await persist(); await browser.close(); }
