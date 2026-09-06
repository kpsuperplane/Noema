import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',results=[];
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const selector=p=>p.getByRole('combobox',{name:'Provider for web.search',exact:true});
const contexts=[];
try {
 const pages=[];
 for(const width of [1440,390]){const c=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});contexts.push(c);pages.push(await c.newPage());}
 const [writer,reader]=pages,events=[];
 reader.on('websocket',ws=>{events.push({event:'opened',path:new URL(ws.url()).pathname});ws.on('framereceived',frame=>{try{const m=JSON.parse(frame.payload);events.push({event:'received',type:m.type,fields:Object.keys(m.payload?.data??{})});}catch{}});ws.on('close',()=>events.push({event:'closed'}));});
 for(const p of pages){p.setDefaultTimeout(15000);await p.goto(origin+'/settings/tools/web');await selector(p).waitFor();assert.equal((await selector(p).innerText()).trim(),'Codex');}
 const change=async value=>{await selector(writer).click();const response=writer.waitForResponse(r=>r.url().endsWith('/graphql')&&r.request().postData()?.includes('SaveWebToolProviderBinding'));await writer.getByRole('option',{name:value,exact:true}).click();const data=await(await response).json();assert.equal(data.errors,undefined);};
 try {
  await change('DuckDuckGo public search');
  let refreshed=true;try{await reader.waitForFunction(()=>[...document.querySelectorAll('[role=combobox]')].some(n=>n.getAttribute('aria-label')==='Provider for web.search'&&n.textContent.trim()==='DuckDuckGo public search'),null,{timeout:15000});}catch{refreshed=false;}
  results.push({check:'Second client updates without reload',passed:refreshed,observed:(await selector(reader).innerText()).trim(),expected:'DuckDuckGo public search',observationLimitMs:15000});
  const fresh=await contexts[1].newPage();await fresh.goto(origin+'/settings/tools/web');await selector(fresh).waitFor();assert.equal((await selector(fresh).innerText()).trim(),'DuckDuckGo public search');results.push({check:'Fresh client sees saved setting',passed:true});await fresh.close();
  await reader.reload();await selector(reader).waitFor();assert.equal((await selector(reader).innerText()).trim(),'DuckDuckGo public search');results.push({check:'Manual reload refreshes stale client',passed:true});
 }finally{
  await change('Codex');await writer.reload();await selector(writer).waitFor();assert.equal((await selector(writer).innerText()).trim(),'Codex');await reader.reload();await selector(reader).waitFor();assert.equal((await selector(reader).innerText()).trim(),'Codex');results.push({check:'Original setting restored and read by both clients',passed:true});
 }
 results.push({websocketEvents:events});
}finally{await browser.close();await writeFile('/var/tmp/noema-suite-run-20260905/cross-client-setting-results.json',JSON.stringify({instance:origin,transport:'Authenticated direct HTTPS origin. Two independent browser contexts share the authorized session.',results},null,2)+'\n');}
console.log(JSON.stringify(results));
