import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:'https://noema.kevinpei.com',transport:'Direct HTTPS origin',observed:[]};
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 const page=await context.newPage();await page.goto('https://noema.kevinpei.com/tasks/new');
 await page.getByLabel('Task title',{exact:true}).fill('Migration audit — preview formats');
 await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
 await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill("# Artifact format audit\n\nPublish exactly four Artifacts with artifact.create_local_file. Use artifact_kind report. Each fixture below specifies one version with exact content. Do not rewrite content or use local filesystem tools.\n\n[\n  {\n    \"title\": \"Audit SVG fallback\",\n    \"filename\": \"fallback.svg\",\n    \"media_type\": \"image/svg+xml\",\n    \"content\": \"<svg xmlns=\\\"http://www.w3.org/2000/svg\\\"><text x=\\\"1\\\" y=\\\"15\\\">SVG audit</text></svg>\"\n  },\n  {\n    \"title\": \"Audit binary fallback\",\n    \"filename\": \"fallback.bin\",\n    \"media_type\": \"application/octet-stream\",\n    \"content\": \"Controlled binary fallback bytes\\n\"\n  },\n  {\n    \"title\": \"Audit plain text\",\n    \"filename\": \"literal.txt\",\n    \"media_type\": \"text/plain\",\n    \"content\": \"<b>Literal caf\u00e9 \u65e5\u672c\u8a9e \ud83e\udded</b>\\n\"\n  },\n  {\n    \"title\": \"Audit PDF preview\",\n    \"filename\": \"preview.pdf\",\n    \"media_type\": \"application/pdf\",\n    \"content\": \"%PDF-1.4\\n1 0 obj\\n<< /Type /Catalog /Pages 2 0 R >>\\nendobj\\n2 0 obj\\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\\nendobj\\n3 0 obj\\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\\nendobj\\n4 0 obj\\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\\nendobj\\n5 0 obj\\n<< /Length 50 >>\\nstream\\nBT /F1 18 Tf 30 130 Td (Noema PDF audit 42) Tj ET\\nendstream\\nendobj\\nxref\\n0 6\\n0000000000 65535 f \\n0000000009 00000 n \\n0000000058 00000 n \\n0000000115 00000 n \\n0000000241 00000 n \\n0000000311 00000 n \\ntrailer\\n<< /Size 6 /Root 1 0 R >>\\nstartxref\\n410\\n%%EOF\\n\"\n  }\n]\n\nIn RESULT.md, link each actual returned download URL. Do not access external services.\n");
 await page.getByRole('button',{name:'Run Now',exact:true}).click();
 await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
 evidence.route=page.url();evidence.taskId=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
 console.log('Started',evidence.route);
 await writeFile(root+'/artifact-format-publish-results.json',JSON.stringify(evidence,null,2)+'\n');
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
 await writeFile(root+'/artifact-format-publish-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();
}
