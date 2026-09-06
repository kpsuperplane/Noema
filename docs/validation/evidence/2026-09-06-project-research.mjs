import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',resume=process.env.AUDIT_RESUME==='1';
const evidence=resume?JSON.parse(await readFile('docs/validation/evidence/2026-09-06-project-research-results.json','utf8')):{instance:origin};
if(resume){evidence.observerTimeouts=(evidence.observerTimeouts||0)+1;delete evidence.error;}
const persist=()=>writeFile('docs/validation/evidence/2026-09-06-project-research-results.json',JSON.stringify(evidence,null,2)+'\n');
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),page=await context.newPage();await page.goto(origin);
 const gql=(query,variables={})=>page.evaluate(async({query,variables})=>{const body=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json();if(body.errors)throw new Error(JSON.stringify(body.errors));return body.data;},{query,variables});
 if(!resume){
 let project=(await gql('mutation($input:CreateProjectInput!){createProject(input:$input){project{projectId revision}}}',{input:{workspaceId:'workspace:personal',name:'Migration audit — cited research',description:'Synthetic research for a software team.',clientMutationId:crypto.randomUUID()}})).createProject.project;
 evidence.projectId=project.projectId;await persist();
 const saveContext=async content=>{
  const document=(await gql('query($id:String!){projectDocument(projectId:$id){digest}}',{id:project.projectId})).projectDocument;
  const saved=(await gql('mutation($input:UpdateProjectDocumentInput!){updateProjectDocument(input:$input){project{projectId revision} document{content digest}}}',{input:{projectId:project.projectId,expectedRevision:project.revision,expectedDocumentDigest:document.digest,content,clientMutationId:crypto.randomUUID()}})).updateProjectDocument;
  project=saved.project;return saved.document;
 };
 await saveContext('# Research context\n\nAudience: software team.\nRelease label: AUDIT_RESEARCH_OLD.');
 const task=(await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId revision generation}}}',{input:{workspaceId:'workspace:personal',projectId:project.projectId,title:'Migration audit — GET and HEAD research',taskDocument:'# Research deliverable\n\nResearch HTTP GET and HEAD semantics from authoritative public sources. Search first, then read at most three primary sources. Write REPORT.md with a concise comparison and exact source URLs supporting material claims. Follow the current Project document audience and quote its current release label in the report. Publish REPORT.md as an Artifact. The final result must identify that Artifact and cite the sources. Complete normal planning and review. Use no external writes. Change no files outside this Task. No clarification is needed.',clientMutationId:crypto.randomUUID()}})).captureTask.task;
 evidence.taskId=task.taskId;
 evidence.currentContext=await saveContext('# Research context\n\nAudience: software team.\nRelease label: AUDIT_RESEARCH_CURRENT.\nKeep the report below 350 words.');await persist();
 await gql('mutation($input:QueueTaskInput!){queueTask(input:$input){task{taskId}}}',{input:{taskId:task.taskId,expectedRevision:task.revision,expectedGeneration:task.generation,clientMutationId:crypto.randomUUID()}});
 }
 const task={taskId:evidence.taskId};
 evidence.final=await page.evaluate(taskId=>new Promise((resolve,reject)=>{
  const ws=new WebSocket('wss://'+location.host+'/graphql/ws','graphql-transport-ws');let done=false;
  const timer=setTimeout(()=>{ws.close();reject(new Error('Research completion timeout'));},300000);
  const check=async()=>{
   if(done)return;
   const body=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId stage{key} resultDocument resultMetadata reviewDocument workspaceFiles{path isDirectory sizeBytes} runs{runId kind status}}}',variables:{id:taskId}})})).json();
   if(body.errors){done=true;clearTimeout(timer);ws.close();reject(new Error('Task observer read failed'));return;}
   if(['done','failed','cancelled','waiting'].includes(body.data.task.stage.key)){done=true;clearTimeout(timer);ws.close();resolve(body.data.task);}
  };
  ws.onopen=()=>ws.send(JSON.stringify({type:'connection_init'}));
  ws.onmessage=event=>{const body=JSON.parse(event.data);if(body.type==='connection_ack'){ws.send(JSON.stringify({id:'audit',type:'subscribe',payload:{query:'subscription($id:String!){taskEvents(taskId:$id){kind}}',variables:{id:taskId}}}));void check();}else if(body.type==='next')void check();};
 }),task.taskId);
 await persist();
 assert.equal(evidence.final.stage.key,'done');
 evidence.report=await readFile('/var/lib/noema-dev/tasks/'+task.taskId.slice(5)+'/REPORT.md','utf8');
 evidence.currentContextUsed=evidence.report.includes('AUDIT_RESEARCH_CURRENT')&&!evidence.report.includes('AUDIT_RESEARCH_OLD');assert.ok(evidence.currentContextUsed);
 assert.ok(evidence.final.runs.some(run=>run.kind==='PLANNER'&&run.status==='COMPLETED'));
 assert.ok(evidence.final.runs.some(run=>run.kind==='REVIEWER'&&run.status==='COMPLETED'));
 await page.goto(origin+'/tasks/'+encodeURIComponent(task.taskId)+'?terminal=all');
 await page.getByRole('button',{name:'Result',exact:true}).click();
 evidence.resultSurface=await page.locator('body').innerText();
}catch(error){evidence.error=String(error);throw error;}
finally{await persist();await browser.close();}
