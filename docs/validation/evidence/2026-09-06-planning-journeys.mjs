import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',root='/var/tmp/noema-suite-run-20260905';
const setup=JSON.parse(await readFile(root+'/planning-source-live.json','utf8')),server=setup.response.data.createMcpServer.server;
const evidence={instance:origin,server,journeys:[]};
const persist=()=>writeFile('docs/validation/evidence/2026-09-06-planning-journeys-results.json',JSON.stringify(evidence,null,2)+'\n');
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();await p.goto(origin);
 const gql=(query,variables={})=>p.evaluate(async({query,variables})=>{const body=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json();if(body.errors)throw new Error(JSON.stringify(body.errors));return body.data;},{query,variables});
 evidence.policy=(await gql('mutation($input:SaveCapabilityConnectionPolicyInput!){saveCapabilityConnectionPolicy(input:$input){connectionId status availableToolCount}}',{input:{kind:'MCP',connectionId:server.mcpServerId,expectedConnectionRevision:server.connectionRevision,expectedPolicyRevision:server.policyRevision,dataSharingPolicy:'allow_automatically',unsafeActionPolicy:'always_ask'}})).saveCapabilityConnectionPolicy;
 evidence.project=(await gql('mutation($input:CreateProjectInput!){createProject(input:$input){project{projectId}}}',{input:{workspaceId:'workspace:personal',name:'Migration audit — daily planning',description:'Synthetic planning sources. No external writes or Task execution are authorized.',clientMutationId:crypto.randomUUID()}})).createProject.project;
 evidence.tasks=[];
 for(const source of [{title:'Audit demo preparation',taskDocument:'Prepare the staging demo. Estimate: 2 hours. Deadline: 2026-09-07 15:00 America/Los_Angeles. Not yet started.'},{title:'Audit acceptance packet',taskDocument:'Prepare an acceptance evidence packet. Estimate: 3 hours. Deadline: 2026-09-11 16:00 America/Los_Angeles. Customer sign-off is missing; do not claim completion.'}]){
  const task=(await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId title}}}',{input:{workspaceId:'workspace:personal',projectId:evidence.project.projectId,...source,clientMutationId:crypto.randomUUID()}})).captureTask.task;evidence.tasks.push(task);
 }
 await persist();
 const common='This is a controlled migration audit. Use the Migration audit planning sources MCP tools audit_messages and audit_calendar, native Tasks linked to Project '+evidence.project.projectId+', and current Memory. Read populated records from each source; do not treat empty data as evidence. Use America/Los_Angeles local time. Do not send messages, book events, change sources, change Memory, or execute Tasks. Cite records and identify missing evidence. ';
 for(const [id,request] of [
  ['JOURNEY-02','Prepare my daily brief for 2026-09-07, bounded from local midnight inclusive to the next local midnight exclusive. Include messages, calendar, native Task priorities and deadlines, and one relevant Memory preference. Identify conflicts and unknowns.'],
  ['JOURNEY-03','Plan the week starting 2026-09-07 through 2026-09-13 inclusive. Use the local-midnight interval ending 2026-09-14 exclusive. Account for recurring occurrence identity, all-day events, overlaps, work capacity, and explicit travel constraints. Normal work capacity is weekdays 09:00–17:00, less one hour for lunch and calendar events. Place the native Tasks before their deadlines only when feasible. State unresolved conflicts instead of silently moving events.']
 ]){
  const marker='AUDIT_'+id.replace('-','_')+'_PLANNING_COMPLETE_20260906';
  const prompt=common+request+' End with '+marker+'.';const record={id,prompt};evidence.journeys.push(record);await persist();
  await p.getByRole('textbox',{name:'Message',exact:true}).fill(prompt);await p.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
  await p.locator('[data-lane="assistant"]').filter({hasText:marker}).last().waitFor({timeout:240000});await p.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:240000});
  const items=(await gql('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
  const user=items.find(item=>item.item.__typename==='UserText'&&item.item.text===prompt);assert.ok(user);record.turnId=user.turnId;
  const turn=items.filter(item=>item.turnId===user.turnId);record.answer=turn.filter(item=>item.item.__typename==='AssistantText').map(item=>item.item.text).join('\n');
  record.tools=turn.filter(item=>item.item.__typename==='Activity'&&item.item.metadata.action?.call_id).map(item=>({name:item.item.metadata.action.name,success:item.item.metadata.action.success}));await persist();
 }
 evidence.receipts=JSON.parse(await readFile(root+'/planning-source-receipts.json','utf8'));
}catch(error){evidence.error=String(error);throw error;}
finally{await persist();await b.close();}
