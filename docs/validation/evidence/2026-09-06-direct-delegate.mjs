import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com',evidence={origin,cases:[]};
const persist=()=>writeFile(root+'/direct-delegate-results.json',JSON.stringify(evidence,null,2)+'\n');
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 for(const [index,zone] of ['UTC'].entries()){
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',timezoneId:zone}),page=await context.newPage();
  const c={zone,title:'Migration audit — direct execution'};evidence.cases.push(c);await persist();
  const gql=async(query,variables)=>{const r=await page.evaluate(async({query,variables})=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json(),{query,variables});assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;};
  const list=async()=>(await gql('{tasks(input:{workspaceId:"workspace:personal",scope:ALL},first:100){edges{node{taskId title}} pageInfo{hasNextPage}}}')).tasks;
  await page.goto(origin);const before=await list();assert.equal(before.pageInfo.hasNextPage,false);c.beforeIds=before.edges.map(e=>e.node.taskId);
  c.marker='AUDIT_DIRECT_EXECUTION_ENUM_FIXED_COMPLETE';c.request='# Direct execution audit\n\nCalculate 8 × 9. Write exactly 72 to RESULT.md. Use no external services. Change no files outside this Task.';c.prompt='Migration audit RUN-02. Delegate this complete request as one autonomous Task. Authorize direct execution without a Planner. Use task.delegate with execution_intent containing the exact request_markdown and complexity simple. Use project kind none. Do not perform the calculation in foreground Chat. Title: '+c.title+'. Request: '+c.request+' After delegation, state the Task ID and end with '+c.marker+'.';
  c.sentAt=new Date().toISOString();await persist();await page.getByRole('textbox',{name:'Message',exact:true}).fill(c.prompt);await page.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
  await page.locator('[data-lane="assistant"]').filter({hasText:c.marker}).last().waitFor({timeout:180000});await page.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:180000});
  const after=await list();assert.equal(after.pageInfo.hasNextPage,false);const added=after.edges.map(e=>e.node).filter(t=>!c.beforeIds.includes(t.taskId));c.added=added;await persist();assert.equal(added.length,1);
  c.task=(await gql('query($id:String!){task(taskId:$id){taskId title revision generation taskDocument stage{key} schedule{scheduledFor timeZone} source{conversationId} runs{runId}}}',{id:added[0].taskId})).task;await persist();
  const transcript=(await gql('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{title status metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
  const user=transcript.find(i=>i.item.__typename==='UserText'&&i.item.text===c.prompt);assert.ok(user);c.turn=transcript.filter(i=>i.turnId===user.turnId);
  assert.equal(c.task.title,c.title);const delegation=c.turn.filter(i=>i.item.__typename==='Activity'&&i.item.metadata.action?.name==='task.delegate'&&i.item.metadata.action?.success===true);assert.equal(delegation.length,1);assert.ok(JSON.stringify(delegation[0]).includes(c.task.taskId));c.result='captured';await persist();console.log('PASS direct delegation captured',c.task.taskId);await context.close();
 }
}catch(error){evidence.error=String(error);throw error;}finally{await persist();await browser.close();}
