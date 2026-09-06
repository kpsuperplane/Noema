import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:'https://noema.kevinpei.com',transport:'Direct HTTPS origin',observed:[]};
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 const page=await context.newPage();await page.goto('https://noema.kevinpei.com');
 evidence.scheduledFor=new Date(Math.ceil((Date.now()+45000)/1000)*1000).toISOString();
 const captured=await page.evaluate(async scheduledFor=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId stage{key} runs{runId}}}}',variables:{input:{workspaceId:'workspace:personal',title:'Migration audit — verified due time',taskDocument:'# Due-time audit\n\nPut the exact text deadline audit passed in RESULT.md. Do not use external services.',schedule:{scheduledFor,timeZone:'UTC',missedRunPolicy:'RUN_ONCE'},clientMutationId:crypto.randomUUID()}}})})).json()),evidence.scheduledFor);
 assert.equal(captured.errors,undefined);const task=captured.data.captureTask.task;assert.equal(task.stage.key,'inbox');assert.equal(task.runs.length,0);
 evidence.taskId=task.taskId;evidence.route='https://noema.kevinpei.com/tasks/'+encodeURIComponent(task.taskId);evidence.beforeDueVerified=true;
 console.log('Started',evidence.route);
 await writeFile(root+'/public-due-results.json',JSON.stringify(evidence,null,2)+'\n');
 const result=await page.evaluate(async taskId=>{
  const observations=[];let socket;
  const query=async()=>{
   const r=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId stage{key} revision generation resultDocument reviewDocument runs{runId kind status queuedAt}}}',variables:{id:taskId}})})).json();
   if(r.errors)throw Error(JSON.stringify(r.errors));return r.data.task;
  };
  return await new Promise((resolve,reject)=>{
   let active=false,dirty=false;
   const timer=setTimeout(()=>{socket?.close();resolve({observations,timeout:true});},180000);
   const read=async()=>{if(active){dirty=true;return;}active=true;try{
    do {dirty=false;const task=await query();const last=observations.at(-1);if(!last||last.revision!==task.revision)observations.push(task);
     if(['done','completed','cancelled','failed'].includes(task.stage.key)){clearTimeout(timer);socket?.close();resolve({observations,timeout:false});return;}
    }while(dirty);
   }catch(e){clearTimeout(timer);socket?.close();reject(e);}finally{active=false;}};
   socket=new WebSocket('wss://noema.kevinpei.com/graphql/ws','graphql-transport-ws');
   socket.onopen=()=>socket.send(JSON.stringify({type:'connection_init'}));
   socket.onmessage=event=>{const m=JSON.parse(event.data);if(m.type==='connection_ack'){socket.send(JSON.stringify({id:'task',type:'subscribe',payload:{query:'subscription($id:String!){taskEvents(taskId:$id){__typename}}',variables:{id:taskId}}}));read();}else if(m.type==='next')read();else if(m.type==='error'){clearTimeout(timer);socket.close();reject(Error(JSON.stringify(m.payload)));}};
   socket.onerror=()=>{clearTimeout(timer);reject(Error('Task subscription failed'));};
  });
 },evidence.taskId);
 evidence.observed=result.observations;evidence.timeout=result.timeout;
 assert.equal(result.timeout,false);const final=result.observations.at(-1);assert.equal(final.stage.key,'done');
 const planners=final.runs.filter(r=>r.kind==='PLANNER');assert.equal(planners.length,1);assert.ok(Date.parse(planners[0].queuedAt)>=Date.parse(evidence.scheduledFor));
 assert.ok(final.resultDocument.includes('deadline audit passed'));evidence.resultVerified=true;
 console.log(JSON.stringify({taskId:evidence.taskId,scheduledFor:evidence.scheduledFor,queuedAt:planners[0].queuedAt,result:'pass'}));
} finally {
 await writeFile(root+'/public-due-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();
}
