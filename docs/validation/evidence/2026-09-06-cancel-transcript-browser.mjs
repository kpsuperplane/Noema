import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com',taskId='task:f08092849e214992666a670b7626aa46',runId='run:84da03c33a30420ebf109a60125bced2';
const message='Task cancelled. The external action may have completed.',stamp='2026-09-06T00:11:30Z';
const nodes=[{itemId:'run_item:audit-call',runId,cursor:'audit:0',sequenceIndex:0,roundIndex:0,kind:'TOOL_CALL',status:'FAILED',correlationId:'audit:cancel-call',parentItemId:null,contentText:'audit.write',payload:{name:'audit.write',arguments:{value:'café 日本語'}},createdAt:stamp,updatedAt:stamp},{itemId:'run_item:audit-result',runId,cursor:'audit:1',sequenceIndex:1,roundIndex:0,kind:'TOOL_RESULT',status:'FAILED',correlationId:'audit:cancel-call',parentItemId:'run_item:audit-call',contentText:'audit.write',payload:{name:'audit.write',success:false,result:{error:'outcome_uncertain',message}},createdAt:stamp,updatedAt:stamp}];
const evidence={origin,mode:'Controlled transcript response on the real app; live Task data remains unchanged.',taskId,runId,nodes,views:[]};
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 for(const viewport of [{width:1440,height:1000},{width:390,height:844}]){
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport}),page=await context.newPage();let replaced=0;
  await page.route('**/graphql',async route=>{const body=route.request().postDataJSON();if(body.operationName==='TasksTaskRunItems'&&body.variables.runId===runId){replaced++;await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({data:{taskRunItems:{edges:nodes.map(node=>({cursor:node.cursor,node})),pageInfo:{endCursor:'audit:1',hasNextPage:false}}}})});}else await route.continue();});
  await page.goto(origin+'/tasks/'+encodeURIComponent(taskId));await page.getByRole('button',{name:'Transcript',exact:true}).click();
  await page.getByRole('button').filter({hasText:'audit.write'}).first().waitFor();
  
  await page.getByRole('button').filter({hasText:'audit.write'}).last().click();
  await page.getByText(message,{exact:true}).waitFor();assert.equal(await page.getByText(message,{exact:true}).count(),1);
  await page.waitForTimeout(500);
  await page.screenshot({path:root+'/cancel-transcript-final-'+viewport.width+'.png'});
  assert.ok(replaced>0);console.log('PASS uncertain message visible',viewport.width);
  evidence.views.push({viewport,replaced,text:(await page.locator('body').innerText()).slice(-7000)});await context.close();
 }
}finally{await writeFile(root+'/cancel-transcript-browser-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
