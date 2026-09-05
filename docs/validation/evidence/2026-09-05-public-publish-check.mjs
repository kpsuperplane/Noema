import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:'https://noema.kevinpei.com',transport:'Direct HTTPS origin',observed:[]};
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 const page=await context.newPage();await page.goto('https://noema.kevinpei.com/tasks/new');
 await page.getByLabel('Task title',{exact:true}).fill('Migration audit — published previews');
 await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill("# Artifact preview audit\n\nPublish two durable local Artifacts with artifact.create_local_file.\n\n1. Title: Migration audit Markdown. Filename: audit-report.md. Media type: text/markdown. Kind: report. Create two versions in order. First content: # Version one\n\nOriginal caf\u00e9 \u65e5\u672c\u8a9e \ud83e\udded\n. Second content: # Version two\n\nUpdated caf\u00e9 \u65e5\u672c\u8a9e \ud83e\udded\n.\n2. Title: Migration audit HTML. Filename: audit-preview.html. Media type: text/html. Kind: report. Use this exact controlled security-test fixture as its single version:\n<h1>Visible audit HTML</h1><p>caf\u00e9 \u65e5\u672c\u8a9e \ud83e\udded</p><script>parent.auditExecuted=true</script><img src=\"https://audit.invalid/image\" onerror=\"parent.auditExecuted=true\"><a href=\"https://audit.invalid/navigation\">Audit link</a><form action=\"https://audit.invalid/form\"><button>Submit</button></form>\n\nDo not execute the HTML or access any external service. This is a preview sandbox test. In RESULT.md, identify both published Artifacts and link their actual returned download URLs. Do not modify files outside this Task.\n");
 await page.getByRole('button',{name:'Run Now',exact:true}).click();
 await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
 evidence.route=page.url();evidence.taskId=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
 console.log('Started',evidence.route);
 await writeFile(root+'/public-publish-results.json',JSON.stringify(evidence,null,2)+'\n');
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
 await writeFile(root+'/public-publish-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();
}
