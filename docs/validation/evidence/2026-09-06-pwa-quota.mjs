import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',results=[];
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'allow',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width:390,height:1000}});
 await context.addInitScript(()=>{Object.defineProperty(navigator,'standalone',{get:()=>true});});
 const page=await context.newPage();page.setDefaultTimeout(20000);
 await page.goto(origin+'/memory/stargazing');await page.locator('[data-slot=memory-surface] article').waitFor();
 const snapshot=async()=>page.evaluate(()=>new Promise(resolve=>{const r=indexedDB.open('noema-pwa');r.onsuccess=()=>{const db=r.result,q=db.transaction('records').objectStore('records').get('snapshot');q.onsuccess=()=>{db.close();resolve(q.result?.value??null)};};}));
 await page.waitForFunction(async()=>new Promise(resolve=>{const r=indexedDB.open('noema-pwa');r.onsuccess=()=>{const db=r.result,q=db.transaction('records').objectStore('records').get('snapshot');q.onsuccess=()=>{db.close();resolve(!!q.result?.value?.lastSync&&JSON.stringify(q.result.value.recentQueries).includes('MemoryPage'));};};}));
 const before=await snapshot();assert.ok(before);
 const memory=async()=>page.evaluate(async()=>{const d=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query AuditQuotaMemory { memoryPage(pageId:"stargazing.md") { id body } }'})})).json();if(d.errors||!d.data?.memoryPage?.body)throw Error('Memory read failed');return d.data.memoryPage;});
 const serverBefore=await memory();
 const releaseBefore=await page.evaluate(async()=>({script:navigator.serviceWorker.controller?.scriptURL,caches:await caches.keys()}));
 assert.ok(releaseBefore.script);assert.ok(releaseBefore.caches.length);
 await context.addInitScript(()=>{window.auditQuotaWrites=[];IDBObjectStore.prototype.put=function(record){window.auditQuotaWrites.push(record?.key);throw new DOMException('Controlled audit quota failure','QuotaExceededError');};});
 await page.reload();await page.locator('[data-slot=memory-surface] article').waitFor();
 await page.waitForFunction(()=>window.auditQuotaWrites.includes('snapshot'));
 assert.equal(JSON.stringify(await snapshot())===JSON.stringify(before),true);
 results.push({check:'Quota failure preserves the exact prior snapshot',passed:true,failedKeys:await page.evaluate(()=>window.auditQuotaWrites)});
 assert.equal(JSON.stringify(await memory())===JSON.stringify(serverBefore),true);results.push({check:'Quota failure preserves populated server Memory',passed:true});
 const releaseAfter=await page.evaluate(async()=>({script:navigator.serviceWorker.controller?.scriptURL,caches:await caches.keys()}));assert.deepEqual(releaseAfter,releaseBefore);
 results.push({check:'The active worker and release cache remain unchanged',passed:true,release:releaseAfter});
 await context.setOffline(true);await page.getByText('Noema is offline.',{exact:true}).waitFor();await page.reload();await page.locator('[data-slot=memory-surface] article').waitFor();
 const heading=page.locator('[data-slot=memory-surface] article').getByRole('heading',{name:'Stargazing',exact:true,includeHidden:true});await heading.waitFor({state:'attached'});
 assert.equal(JSON.stringify(await snapshot())===JSON.stringify(before),true);results.push({check:'Offline reload retains the saved article and exact snapshot after quota failures',passed:true});
}catch(e){results.push({check:'Execution stopped',error:e.message.slice(0,500)});}
finally{await browser.close();await writeFile('/var/tmp/noema-suite-run-20260905/pwa-quota-results.json',JSON.stringify({instance:origin,setup:'Phone-width Chromium with installed display emulation, real worker and IndexedDB. Writes throw a controlled QuotaExceededError after saving a real snapshot.',results},null,2)+'\n');}
console.log(JSON.stringify(results));
