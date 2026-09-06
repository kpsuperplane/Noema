import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const taskId = 'task:9496e6cdadbccf4f2f4b0ebe64002590';
const evidence = [];
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
  for (const width of [1440, 390]) {
    const context = await browser.newContext({ serviceWorkers: 'block', storageState: '/var/tmp/noema-audit-credentials/browser-session.json', viewport: { width, height: 1000 } });
    const page = await context.newPage();
    let failQuery = true, failedQueries = 0, failedSaves = 0;
    await page.route('**/graphql', async route => {
      const body = route.request().postDataJSON();
      if (failQuery && body.operationName === 'TasksTaskDetail') {
        failedQueries++;
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ errors: [{ message: 'Audit controlled query failure' }], data: { task: null } }) });
      }
      if (/\bmutation\b/.test(body.query || '')) {
        failedSaves++;
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ errors: [{ message: 'Audit controlled save failure' }], data: null }) });
      }
      await route.continue();
    });
    await page.goto(origin + '/tasks/' + encodeURIComponent(taskId) + '?terminal=all');
    await page.getByText('Task details could not be loaded.', { exact: true }).waitFor();
    assert.ok(failedQueries > 0);
    const retryControls = await page.getByRole('button', { name: /retry|reload|try again/i }).allTextContents();
    await page.screenshot({ path: `/var/tmp/noema-suite-run-20260905/query-failure-${width}.png`, animations: 'disabled' });
    failQuery = false;
    await page.reload();
    await page.getByRole('button', { name: 'Edit description', exact: true }).waitFor();
    const readTask = () => page.evaluate(async id => {
      const response = await fetch('/graphql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query: 'query($id:String!){task(taskId:$id){taskDocument revision}}', variables: { id } }) });
      const body = await response.json();
      if (body.errors) throw new Error('Task read failed');
      return body.data.task;
    }, taskId);
    const before = await readTask();
    await page.getByRole('button', { name: 'Edit description', exact: true }).click();
    await page.getByRole('button', { name: 'Edit source', exact: true }).click();
    const input = page.getByRole('textbox', { name: 'Task description Markdown source', exact: true });
    const draft = '# Unsaved failure audit\n\nPreserve café 日本語 🧭 exactly.\n';
    await input.fill(draft);
    await page.getByRole('button', { name: 'Save description', exact: true }).click();
    await page.getByText('Audit controlled save failure', { exact: false }).first().waitFor();
    assert.equal(await input.inputValue(), draft);
    assert.equal(await page.getByRole('button', { name: 'Save description', exact: true }).isEnabled(), true);
    await page.getByRole('button', { name: 'Save description', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('button[aria-label="Save description"]')?.disabled);
    assert.equal(await input.inputValue(), draft);
    assert.equal(failedSaves, 2);
    assert.deepEqual(await readTask(), before);
    await page.screenshot({ path: `/var/tmp/noema-suite-run-20260905/save-failure-${width}.png`, animations: 'disabled' });
    await page.getByRole('button', { name: 'Cancel Description edit', exact: true }).click();
    assert.deepEqual(await readTask(), before);
    evidence.push({ width, failedQueries, queryErrorVisible: true, retryControls, browserReloadRecovers: true, failedSaves, exactDraftPreserved: true, saveRemainsAvailable: true, serverUnchanged: true });
    await context.close();
  }
} finally {
  await writeFile('docs/validation/evidence/2026-09-06-query-save-failures-results.json', JSON.stringify({ instance: origin, taskId, scope: 'Controlled HTTP failures. Same-hostname origin access. No server mutations. Successful save retry, authentication, and provider draft recovery are not covered.', evidence }, null, 2) + '\n');
  await browser.close();
}
