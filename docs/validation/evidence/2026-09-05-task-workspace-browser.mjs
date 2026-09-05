import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile,stat,chown} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',taskId='task:045fa8e6f2132876904ccf16db0c5bf5',directory='/var/lib/noema-dev/tasks/'+taskId.slice(5);
const owner=await stat(directory),fixtures={'audit-support.md':'# Support source\n\nDistinct café 日本語 🧭 support.\n','audit-notes.txt':'<b>Literal support café 日本語 🧭</b>\n'};
for(const [name,content] of Object.entries(fixtures)){await writeFile(directory+'/'+name,content,{flag:'wx',mode:0o600});await chown(directory+'/'+name,owner.uid,owner.gid);}
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});const evidence=[];
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();await page.goto('https://noema.kevinpei.com/tasks/'+encodeURIComponent(taskId)+'?terminal=all');
 await page.getByText('café 日本語 🧭',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Task',exact:true}).click();await page.getByRole('heading',{name:'Calculation audit',exact:true}).waitFor();
 await page.getByRole('button',{name:'Review',exact:true}).click();await page.getByText('RESULT.md satisfies all requirements:',{exact:false}).waitFor();
 await page.getByRole('button',{name:'audit-support',exact:true}).click();await page.getByRole('heading',{name:'Support source',exact:true}).waitFor();assert.ok((await page.getByRole('region',{name:'Task workspace',exact:true}).innerText()).includes('Distinct café 日本語 🧭 support.'));
 await page.getByRole('button',{name:'audit-notes.txt',exact:true}).click();const text=page.getByRole('region',{name:'Task workspace',exact:true}).locator('pre');await text.waitFor();assert.equal(await text.textContent(),fixtures['audit-notes.txt']);assert.equal(await text.locator('b').count(),0);
 await page.getByRole('button',{name:'Transcript',exact:true}).click();const region=page.getByRole('region',{name:'Transcript',exact:true});await region.getByRole('log').evaluate(el=>{el.scrollTop=0;});await region.getByText('Task Planner · Planner · Running',{exact:true}).waitFor();
 evidence.push({id:'TASK-08',result:'pass',detail:'Reopen selects the completed result. Request, review, Markdown support, and literal text support render distinct content. Transcript navigation reaches the Planner start.'});console.log('PASS TASK-08');
}finally{await writeFile(root+'/task-workspace-browser-results.json',JSON.stringify({instance:'https://noema.kevinpei.com',taskId,fixtures,evidence},null,2)+'\n');await browser.close();}
