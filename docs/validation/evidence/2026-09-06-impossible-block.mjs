import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const evidence={origin,cases:[]},persist=()=>writeFile(root+'/impossible-block-results.json',JSON.stringify(evidence,null,2)+'\n');
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width:1440,height:1000}});
 const page=await context.newPage();
 for(const [key,title,request] of [
 ['impossible','Migration audit — impossible integer','# Integer request\n\nProduce an integer that is both odd and even under the standard definitions. If this is impossible, explain the limitation in RESULT.md. This is a mathematical request, not a request for alternative definitions. Use no external services. Change no files outside this Task.'],
 ['human-input','Migration audit — missing delivery address','# Delivery label\n\nPrepare a delivery label using the delivery address I will provide. Ask me for the address before creating the label. Do not assume an address. Do not contact anyone or use external services. Change no files outside this Task.']]) {
 const c={key,title,request,startedAt:new Date().toISOString()};evidence.cases.push(c);
 await page.goto(origin+'/tasks/new');
 await page.getByLabel('Task title',{exact:true}).fill(title);
 await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill(request);
 await page.getByRole('button',{name:'Run Now',exact:true}).click();
 await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
 c.route=page.url();c.taskId=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());await persist();console.log('START',key,c.taskId);
 const result=await page.evaluate(async taskId=>{
  const observations=[];let socket;
  const query=async()=>{
   const r=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId stage{key} revision generation resultDocument reviewDocument activeGate{gateId kind prompt} runs{runId kind status}}}',variables:{id:taskId}})})).json();
   if(r.errors)throw Error(JSON.stringify(r.errors));return r.data.task;
  };
  return await new Promise((resolve,reject)=>{
   let active=false,dirty=false;
   const timer=setTimeout(()=>{socket?.close();resolve({observations,timeout:true});},240000);
   const read=async()=>{if(active){dirty=true;return;}active=true;try{
    do {dirty=false;const task=await query();const last=observations.at(-1);if(!last||last.revision!==task.revision)observations.push(task);
     if(['done','completed','cancelled','failed','waiting'].includes(task.stage.key)){clearTimeout(timer);socket?.close();resolve({observations,timeout:false});return;}
    }while(dirty);
   }catch(e){clearTimeout(timer);socket?.close();reject(e);}finally{active=false;}};
   socket=new WebSocket('wss://noema.kevinpei.com/graphql/ws','graphql-transport-ws');
   socket.onopen=()=>socket.send(JSON.stringify({type:'connection_init'}));
   socket.onmessage=event=>{const m=JSON.parse(event.data);if(m.type==='connection_ack'){socket.send(JSON.stringify({id:'task',type:'subscribe',payload:{query:'subscription($id:String!){taskEvents(taskId:$id){__typename}}',variables:{id:taskId}}}));read();}else if(m.type==='next')read();else if(m.type==='error'){clearTimeout(timer);socket.close();reject(Error(JSON.stringify(m.payload)));}};
   socket.onerror=()=>{clearTimeout(timer);reject(Error('Task subscription failed'));};
  });
 },c.taskId);
 c.observations=result.observations;c.timeout=result.timeout;await persist();console.log('STATE',key,JSON.stringify(result.observations.at(-1)));
 for(const viewport of [{width:1440,height:1000},{width:390,height:844}]) {
 await page.setViewportSize(viewport);await page.goto(c.route);await page.getByRole('button',{name:'Workspace',exact:true}).waitFor();
 await page.waitForTimeout(700);c['text'+viewport.width]=await page.locator('body').innerText();await page.screenshot({path:root+'/'+key+'-'+viewport.width+'.png'});
 }
 await persist();
 }
} catch(error){evidence.error=String(error);throw error;} finally {await persist();await browser.close();}
