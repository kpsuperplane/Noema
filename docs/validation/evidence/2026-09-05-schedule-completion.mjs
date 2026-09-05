import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',evidence=JSON.parse(await readFile(root+'/schedule-browser-results.json','utf8'));
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width:1440,height:1000}}),page=await context.newPage();await page.goto(evidence.origin+'/tasks/'+encodeURIComponent(evidence.taskId));
 const final=await page.evaluate(async id=>{
  const query=async()=>{const r=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId title taskDocument stage{key} completedAt schedule{scheduledFor} runs{runId kind status}}}',variables:{id}})})).json();if(r.errors)throw new Error(JSON.stringify(r.errors));return r.data.task;};
  return new Promise((resolve,reject)=>{const ws=new WebSocket('wss://'+location.host+'/graphql/ws','graphql-transport-ws');const timer=setTimeout(()=>{ws.close();reject(new Error('Task completion timeout'));},240000);const check=async()=>{try{const t=await query();if(['done','cancelled','failed'].includes(t.stage.key)){clearTimeout(timer);ws.close();resolve(t);}}catch(e){clearTimeout(timer);ws.close();reject(e);}};ws.onopen=()=>ws.send(JSON.stringify({type:'connection_init'}));ws.onmessage=e=>{const m=JSON.parse(e.data);if(m.type==='connection_ack'){ws.send(JSON.stringify({id:'audit',type:'subscribe',payload:{query:'subscription($id:String!){taskEvents(taskId:$id){kind}}',variables:{id}}}));void check();}else if(m.type==='next')void check();else if(m.type==='error'){clearTimeout(timer);ws.close();reject(new Error(JSON.stringify(m)));}};});
 },evidence.taskId);
 assert.equal(final.stage.key,'done');assert.ok(Date.parse(final.completedAt)<Date.parse(final.schedule.scheduledFor));assert.equal(final.runs.filter(r=>r.kind==='EXECUTOR').length,1);assert.ok(final.runs.every(r=>r.status==='COMPLETED'));
 const result=await readFile('/var/lib/noema-dev/tasks/'+evidence.taskId.slice(5)+'/RESULT.md','utf8');assert.match(result,/56/);evidence.completion={task:final,result};
 await page.reload();await page.getByRole('button',{name:'Workspace',exact:true}).click();await page.getByRole('button',{name:'Result',exact:true}).click();await page.getByText('56',{exact:true}).first().waitFor();await page.screenshot({path:root+'/schedule-completed-desktop.png'});evidence.completion.browserResultVisible=true;
 await writeFile(root+'/schedule-browser-results.json',JSON.stringify(evidence,null,2));console.log('PASS early completion',final.completedAt);
}finally{await browser.close();}
