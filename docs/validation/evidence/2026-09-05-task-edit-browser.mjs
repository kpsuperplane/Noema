import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile,stat} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const browser=await chromium.launch({channel:'chromium',headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});const evidence=[];let taskId,route;
try {
 const options={serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'};
 const first=await browser.newContext(options),second=await browser.newContext(options),a=await first.newPage(),b=await second.newPage();
 await a.goto(origin+'/tasks/new');await a.getByLabel('Task title',{exact:true}).fill('Migration audit — competing drafts');await a.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 const original='# Original request\n\nPreserve café 日本語 🧭.\n';await a.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill(original);await a.getByRole('button',{name:'More Task creation actions',exact:true}).click();await a.getByRole('menuitem',{name:'Add to Inbox',exact:true}).click();await a.waitForFunction(()=>!location.pathname.endsWith('/new'));
 route=a.url();taskId=decodeURIComponent(new URL(route).pathname.split('/').pop());await writeFile(root+'/task-edit-source.json',JSON.stringify({taskId,route})+'\n');
 const directory='/var/lib/noema-dev/tasks/'+taskId.slice(5),before=await stat(directory);
 const task=async()=>{const r=await a.evaluate(async id=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){title taskDocument revision stage{key}}}',variables:{id}})})).json()),taskId);assert.equal(r.errors,undefined);return r.data.task;};
 await a.getByRole('button',{name:'Edit task title',exact:true}).click();await a.getByRole('textbox',{name:'task title',exact:true}).fill('Migration audit — edited café 日本語 🧭');await a.getByRole('button',{name:'Save task title',exact:true}).click();await a.getByRole('heading',{name:'Migration audit — edited café 日本語 🧭',exact:true}).waitFor();
 await a.reload();assert.equal((await task()).title,'Migration audit — edited café 日本語 🧭');assert.equal((await stat(directory)).ino,before.ino);
 const edit=async p=>{await p.getByRole('button',{name:'Edit description',exact:true}).click();await p.getByRole('button',{name:'Edit source',exact:true}).click();return p.getByRole('textbox',{name:'Task description Markdown source',exact:true});};
 const rich='# Updated request\n\n**Bold café** and [exact URL](https://example.com/a?x=1&y=2).\n\n- First\n- 日本語 🧭\n\n```json\n{"ordinary":"kept"}\n```\n';
 const source=await edit(a);await source.fill(rich);await a.getByRole('button',{name:'Save description',exact:true}).click();await a.getByRole('heading',{name:'Updated request',exact:true}).waitFor();await a.reload();assert.equal((await task()).taskDocument,rich);assert.equal(await readFile(directory+'/TASK.md','utf8'),rich);
 evidence.push({id:'TASK-03',result:'pass',detail:'Browser title and Markdown edits survive reload with exact Unicode content. The allocated Task directory remains the same.'});console.log('PASS TASK-03');
 await b.goto(route);const draftA=await edit(a),draftB=await edit(b);const local='# Unsaved second draft\n\nKeep this café 日本語 🧭.\n';await draftB.fill(local);const current=rich+'\nAccepted first-client change.\n';await draftA.fill(current);await a.getByRole('button',{name:'Save description',exact:true}).click();
 await b.getByRole('button',{name:'Reload latest',exact:true}).waitFor();assert.equal(await draftB.inputValue(),local);assert.equal((await task()).taskDocument,current);
 await b.getByRole('button',{name:'Reload latest',exact:true}).click();assert.equal(await b.getByRole('textbox',{name:'Task description Markdown source',exact:true}).inputValue(),local);
 await b.getByRole('button',{name:'Cancel Description edit',exact:true}).click();await b.getByText('Accepted first-client change.',{exact:true}).waitFor();assert.equal((await task()).taskDocument,current);
 evidence.push({id:'TASK-04',result:'pass',detail:'A competing save marks the second browser stale. Its local draft remains intact. Reload acknowledges current authority; cancel reveals the accepted server content without overwriting it.'});console.log('PASS TASK-04');
 const cancelled=await edit(a);await cancelled.fill('Discard this draft only.');await a.getByRole('button',{name:'Cancel Description edit',exact:true}).click();assert.equal((await task()).taskDocument,current);
 const roundtrip=await edit(a);assert.equal(await roundtrip.inputValue(),current);await a.getByRole('button',{name:'Use rich editor',exact:true}).click();await a.getByRole('button',{name:'Edit source',exact:true}).click();assert.equal(await a.getByRole('textbox',{name:'Task description Markdown source',exact:true}).inputValue(),current);
 await a.getByRole('button',{name:'Cancel Description edit',exact:true}).click();
 evidence.push({id:'TASK-05-partial',result:'pass',detail:'Source/rich switching preserves source. Cancel discards only the unsaved draft. Forced rich-editor loading failure remains pending.'});console.log('PASS TASK-05-partial');
}finally{await writeFile(root+'/task-edit-browser-results.json',JSON.stringify({instance:origin,taskId,route,evidence},null,2)+'\n');await browser.close();}
