import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 const page=await context.newPage();await page.goto('https://noema.kevinpei.com/tasks/new');
 await page.getByLabel('Task title',{exact:true}).fill('Migration audit — Unicode café 日本語 🧭');
 await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 const doc='# Migration audit\n\nVerify that café, 日本語, 🧭, and https://example.com/a?b=1 remain intact.\n';
 await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill(doc);
 await page.getByRole('button',{name:'More Task creation actions',exact:true}).click();
 await page.getByRole('menuitem',{name:'Add to Inbox',exact:true}).click();
 await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
 const taskId=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
 const result=await page.evaluate(async taskId=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){title taskDocument stage{key}}}',variables:{id:taskId}})})).json()),taskId);
 assert.equal(result.errors,undefined);assert.equal(result.data.task.taskDocument,doc);assert.equal(result.data.task.stage.key,'inbox');
 const evidence={instance:'https://noema.kevinpei.com',route:page.url(),taskId,result:'pass',case:'TASK-02',detail:'Browser capture preserves exact Unicode Markdown in Inbox.',transport:'Direct HTTPS origin on the same host; Cloudflare Access edge not covered.'};
 await writeFile('/var/tmp/noema-suite-run-20260905/public-task-results.json',JSON.stringify(evidence,null,2)+'\n');
 console.log(JSON.stringify(evidence));
} finally {await browser.close();}
