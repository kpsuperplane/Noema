import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin = 'https://noema.kevinpei.com';
const evidence = { instance: origin, scope: 'Keyboard-only Task capture and title editing with reverse Tab traversal after direct navigation. No screen reader, login, or approval check.', clients: [] };
const browser = await chromium.launch({ headless: true, args: ['--no-sandbox', '--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1'] });
try {
 for (const width of [1440,390]) {
  const context = await browser.newContext({ serviceWorkers: 'block', storageState: '/var/tmp/noema-audit-credentials/browser-session.json', viewport: { width, height: 1000 } });
  const page = await context.newPage(), record = { width, focus: [] };
  evidence.clients.push(record);
  const reach = async (role, name) => {
   const target = page.getByRole(role, { name, exact: true });
   await target.waitFor();
   record.lastTraversal = { target: name, steps: [] };
   for (let step = 0; step < 80; step++) {
    if (await target.count() && await target.evaluate(element => element === document.activeElement)) {
     record.focus.push(await target.evaluate(element => { const style = getComputedStyle(element), rect = element.getBoundingClientRect(); return { name: element.getAttribute('aria-label') || element.textContent || element.getAttribute('placeholder'), outline: style.outline, boxShadow: style.boxShadow, focusVisible: element.matches(':focus-visible'), inViewport: rect.top >= 0 && rect.bottom <= innerHeight }; }));
     return true;
    }
    if (step < 12) record.lastTraversal.steps.push(await page.evaluate(() => { const e=document.activeElement; return { tag:e.tagName, role:e.getAttribute('role'), label:e.getAttribute('aria-label'), editable:e.getAttribute('contenteditable'), tabIndex:e.tabIndex }; }));
    await page.keyboard.press('Shift+Tab');
   }
   await page.screenshot({path:`/var/tmp/noema-suite-run-20260905/keyboard-reverse-traversal-${width}.png`,animations:'disabled'});
   record.blockedTarget = name;
   record.escapeChecks = [];
   for (const key of ['Shift+Tab','Escape','Tab']) {
    await page.keyboard.press(key);
    record.escapeChecks.push(await page.evaluate(key => { const e=document.activeElement; return {key,tag:e.tagName,role:e.getAttribute('role'),label:e.getAttribute('aria-label'),editable:e.getAttribute('contenteditable')}; },key));
   }
   return false;
  };
  await page.goto(origin + '/tasks/new');
  await reach('textbox','Task title');
  const title = 'Migration audit — reverse keyboard ' + width;
  await page.keyboard.insertText(title);
  if (!await reach('button','Edit Markdown source')) { await context.close(); continue; }
  await page.keyboard.press('Enter');
  await reach('textbox','Task document Markdown source');
  await page.keyboard.insertText('# Keyboard audit\n\nPreserve café 日本語 🧭.');
  await reach('button','More Task creation actions');
  await page.keyboard.press('Enter');
  const inbox = page.getByRole('menuitem', { name: 'Add to Inbox', exact: true });
  await inbox.waitFor();
  if (!await inbox.evaluate(element => element === document.activeElement)) await page.keyboard.press('ArrowDown');
  assert.equal(await inbox.evaluate(element => element === document.activeElement),true);
  await page.keyboard.press('Enter');
  await page.waitForURL(url => !url.pathname.endsWith('/new'));
  record.taskId = decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
  await reach('button','Edit task title');
  await page.keyboard.press('Enter');
  await reach('textbox','task title');
  await page.keyboard.press('ControlOrMeta+A');
  const edited = title + ' saved';
  await page.keyboard.insertText(edited);
  await reach('button','Save task title');
  await page.screenshot({ path: `/var/tmp/noema-suite-run-20260905/keyboard-reverse-save-${width}.png`, animations: 'disabled' });
  await page.keyboard.press('Enter');
  await page.getByRole('heading',{name:edited,exact:true}).waitFor();
  await page.reload();
  await page.getByRole('heading',{name:edited,exact:true}).waitFor();
  record.savedTitleSurvivesReload = true;
  await context.close();
 }
} catch(error) { evidence.error = String(error); throw error; }
finally { await writeFile('docs/validation/evidence/2026-09-06-task-keyboard-reverse-results.json',JSON.stringify(evidence,null,2)+'\n'); await browser.close(); }
