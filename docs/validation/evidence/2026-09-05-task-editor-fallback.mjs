import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',source=JSON.parse(await readFile(root+'/task-edit-source.json','utf8'));
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});let evidence;
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});let blocked=0;
 await context.route('**/MarkdownEditorImpl-*.js',r=>{blocked++;return r.abort('failed');});
 const page=await context.newPage();await page.goto(source.route);
 const read=()=>page.evaluate(async id=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskDocument}}',variables:{id}})})).json()),source.taskId);
 const before=(await read()).data.task.taskDocument;
 await page.getByRole('button',{name:'Edit description',exact:true}).click();await page.getByText('Rich editing is unavailable. Markdown source remains editable.',{exact:true}).waitFor();assert.ok(blocked>0);
 const input=page.getByRole('textbox',{name:'Task description Markdown source',exact:true});assert.equal(await input.inputValue(),before);
 const after=before+'\nSource fallback saved café 日本語 🧭.\n';await input.fill(after);await page.getByRole('button',{name:'Save description',exact:true}).click();await page.getByText('Source fallback saved café 日本語 🧭.',{exact:true}).waitFor();assert.equal((await read()).data.task.taskDocument,after);
 evidence={result:'pass',case:'TASK-05-editor-load',taskId:source.taskId,blockedModuleRequests:blocked,detail:'A controlled rich-editor module load failure exposes the original editable source. Saving preserves the exact new Unicode source.'};console.log('PASS TASK-05-editor-load');
}finally{await writeFile(root+'/task-editor-fallback-results.json',JSON.stringify(evidence??{result:'incomplete'},null,2)+'\n');await browser.close();}
