import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:'https://noema.kevinpei.com',transport:'Direct HTTPS origin',observed:[]};
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 const page=await context.newPage();await page.goto('https://noema.kevinpei.com/tasks/new');
 await page.getByLabel('Task title',{exact:true}).fill('Migration audit — calculation and review');
 await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill('# Calculation audit\n\nCalculate 17 + 25 and 25 percent of 2400. Put both answers in RESULT.md. Include the exact text café 日本語 🧭. Do not access external services or change files outside this Task.');
 await page.getByRole('button',{name:'Run Now',exact:true}).click();
 await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
 evidence.route=page.url();evidence.taskId=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
 console.log('Started',evidence.route);
 await writeFile(root+'/public-run-results.json',JSON.stringify(evidence,null,2)+'\n');
 const result=await page.evaluate(async taskId=>{
  const observations=[];let socket;
  const query=async()=>{
   const r=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId stage{key} revision generation resultDocument reviewDocument}}',variables:{id:taskId}})})).json();
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
 console.log(JSON.stringify(result));
} finally {
 await writeFile(root+'/public-run-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();
}
