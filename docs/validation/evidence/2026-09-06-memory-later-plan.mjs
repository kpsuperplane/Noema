import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const marker = 'AUDIT_LATER_MEMORY_PLAN_20260906';
const prompt = 'Migration audit JOURNEY-10. Use search_memory and read_memory_page to retrieve my current personal travel-note preference and lasting leisure interest. Prepare a practical three-step plan for a quiet evening on a future trip. Include a short travel-note template in my current preferred language. Explain which current Memory facts shaped the plan and cite their evidence. Do not assume a destination, date, weather, booking, or availability. Do not use external services, change Memory, or create a Task. End with ' + marker + '.';
const evidence = { instance: origin, prompt, reusedCorrection: '2026-09-05-memory-correction-results.json', correctionSource: 'item:7c3f80acd8a9307fb874242e8a4b3b2c' };
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
  const context = await browser.newContext({ serviceWorkers: 'block', storageState: '/var/tmp/noema-audit-credentials/browser-session.json' });
  const page = await context.newPage();
  await page.goto(origin);
  const gql = (query, variables = {}) => page.evaluate(async ({ query, variables }) => {
    const body = await (await fetch('/graphql', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ query, variables }) })).json();
    if (body.errors) throw new Error('Audit GraphQL read failed');
    return body.data;
  }, { query, variables });
  const read = async () => (await gql('query{memoryPage(pageId:"travel-notes.md"){id body citations{sources{source kind excerpt}}}}')).memoryPage;
  const before = await read();
  evidence.currentMemoryHasFrench = before.body.includes('French');
  evidence.currentMemoryCitesCorrection = before.citations.some(citation => citation.sources.some(source => source.source === evidence.correctionSource && source.kind === 'HUMAN_MESSAGE'));
  assert.ok(evidence.currentMemoryHasFrench && evidence.currentMemoryCitesCorrection);
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill(prompt);
  await page.getByRole('textbox', { name: 'Message', exact: true }).press('Enter');
  await page.locator('[data-lane="assistant"]').filter({ hasText: marker }).last().waitFor({ timeout: 180000 });
  await page.getByRole('button', { name: 'Send message', exact: true }).waitFor({ timeout: 180000 });
  const items = (await gql('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId metadata item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{title status metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
  const users = items.filter(item => item.item.__typename === 'UserText' && item.item.text === prompt);
  assert.equal(users.length, 1);
  evidence.turnId = users[0].turnId;
  const turn = items.filter(item => item.turnId === evidence.turnId);
  evidence.answers = turn.filter(item => item.item.__typename === 'AssistantText').map(item => ({ itemId: item.itemId, text: item.item.text, metadata: item.metadata }));
  evidence.tools = turn.filter(item => item.item.__typename === 'Activity' && item.item.metadata.action?.call_id).map(item => ({ name: item.item.metadata.action.name, success: item.item.metadata.action.success }));
  assert.ok(evidence.tools.some(tool => tool.name === 'search_memory'));
  assert.ok(evidence.tools.some(tool => tool.name === 'read_memory_page'));
  assert.ok(evidence.tools.every(tool => tool.success === true));
  evidence.memoryUnchanged = JSON.stringify(await read()) === JSON.stringify(before);
  assert.ok(evidence.memoryUnchanged);
} catch (error) { evidence.error = String(error); throw error; }
finally {
  await writeFile('docs/validation/evidence/2026-09-06-memory-later-plan-results.json', JSON.stringify(evidence, null, 2) + '\n');
  await browser.close();
}
