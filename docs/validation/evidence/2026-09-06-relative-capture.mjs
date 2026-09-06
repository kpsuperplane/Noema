import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com',evidence={origin,cases:[]};
const persist=()=>writeFile(root+'/relative-capture-results.json',JSON.stringify(evidence,null,2)+'\n');
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 for(const [index,zone] of ['America/Los_Angeles','Asia/Tokyo'].entries()){
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',timezoneId:zone}),page=await context.newPage();
  const c={zone,title:'Migration audit — relative tomorrow '+zone};evidence.cases.push(c);await persist();
  const gql=async(query,variables)=>{const r=await page.evaluate(async({query,variables})=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json(),{query,variables});assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;};
  const list=async()=>(await gql('{tasks(input:{workspaceId:"workspace:personal",scope:ALL},first:100){edges{node{taskId title}} pageInfo{hasNextPage}}}')).tasks;
  await page.goto(origin);const before=await list();assert.equal(before.pageInfo.hasNextPage,false);c.beforeIds=before.edges.map(e=>e.node.taskId);
  c.marker='AUDIT_RELATIVE_CAPTURE_'+index+'_COMPLETE';c.prompt='Migration audit TIME-03. Create exactly one scheduled Task for tomorrow at 09:00 in my current timezone. Title: '+c.title+'. Task document: "Calculate 6 × 7 and write only 42 to RESULT.md." Do not run it now. Do not access external services. After capture, state the saved local date, time, timezone and Task ID, then end with '+c.marker+'.';
  c.sentAt=new Date().toISOString();await persist();await page.getByRole('textbox',{name:'Message',exact:true}).fill(c.prompt);await page.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
  await page.locator('[data-lane="assistant"]').filter({hasText:c.marker}).last().waitFor({timeout:180000});await page.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:180000});
  const after=await list();assert.equal(after.pageInfo.hasNextPage,false);const added=after.edges.map(e=>e.node).filter(t=>!c.beforeIds.includes(t.taskId));c.added=added;await persist();assert.equal(added.length,1);
  c.task=(await gql('query($id:String!){task(taskId:$id){taskId title revision generation taskDocument stage{key} schedule{scheduledFor timeZone} source{conversationId} runs{runId}}}',{id:added[0].taskId})).task;await persist();
  const transcript=(await gql('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{title status metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
  const user=transcript.find(i=>i.item.__typename==='UserText'&&i.item.text===c.prompt);assert.ok(user);c.turn=transcript.filter(i=>i.turnId===user.turnId);
  const localDate=stamp=>new Intl.DateTimeFormat('en-CA',{timeZone:zone,year:'numeric',month:'2-digit',day:'2-digit'}).format(new Date(stamp));
  const requestDate=localDate(c.sentAt),tomorrow=new Date(requestDate+'T00:00:00Z');tomorrow.setUTCDate(tomorrow.getUTCDate()+1);c.expectedLocalDate=tomorrow.toISOString().slice(0,10);
  c.actualLocalDate=localDate(c.task.schedule.scheduledFor);c.actualLocalTime=new Intl.DateTimeFormat('en-GB',{timeZone:zone,hour:'2-digit',minute:'2-digit',hourCycle:'h23'}).format(new Date(c.task.schedule.scheduledFor));
  assert.equal(c.task.title,c.title);assert.equal(c.task.schedule.timeZone,zone);assert.equal(c.actualLocalDate,c.expectedLocalDate);assert.equal(c.actualLocalTime,'09:00');assert.equal(c.task.runs.length,0);
  await page.goto(origin+'/tasks/'+encodeURIComponent(c.task.taskId));await page.getByRole('heading',{name:c.title,exact:true}).waitFor();await page.screenshot({path:root+'/relative-capture-'+index+'.png'});
  c.cancelled=(await gql('mutation($input:CancelTaskInput!){cancelTask(input:$input){task{taskId stage{key} runs{runId}}}}',{input:{taskId:c.task.taskId,expectedRevision:c.task.revision,expectedGeneration:c.task.generation,clientMutationId:crypto.randomUUID(),reason:'Relative-time audit completed.'}})).cancelTask.task;assert.equal(c.cancelled.stage.key,'cancelled');assert.equal(c.cancelled.runs.length,0);c.result='pass';await persist();console.log('PASS',zone,c.task.schedule.scheduledFor);await context.close();
 }
}catch(error){evidence.error=String(error);throw error;}finally{await persist();await browser.close();}
