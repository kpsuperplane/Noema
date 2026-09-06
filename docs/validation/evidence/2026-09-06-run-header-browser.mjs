import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com',taskId='task:f08092849e214992666a670b7626aa46',evidence={origin,taskId,views:[]};
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 for(const viewport of [{width:1440,height:1000},{width:390,height:844}]){
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport}),page=await context.newPage();
  await page.goto(origin+'/tasks/'+encodeURIComponent(taskId));await page.getByRole('button',{name:'Transcript',exact:true}).click();
  const data=await page.evaluate(async taskId=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){task(taskId:$id){taskId stage{key} runs{runId kind status}}}',variables:{id:taskId}})})).json(),taskId);
  assert.equal(data.errors,undefined);assert.equal(data.data.task.stage.key,'done');assert.equal(data.data.task.runs.length,2);assert.ok(data.data.task.runs.every(r=>r.status==='COMPLETED'));
  for(const label of ['Task Executor · Executor · Completed','Task Reviewer · Reviewer · Completed'])await page.getByText(label,{exact:true}).waitFor();
  assert.equal(await page.getByText(/Task (Executor|Reviewer) · (Executor|Reviewer) · Running/).count(),0);
  await page.waitForTimeout(500);await page.screenshot({path:root+'/run-header-'+viewport.width+'.png'});evidence.views.push({viewport,task:data.data.task,completedLabelsVisible:true,runningLabelsAbsent:true});console.log('PASS completed run headers',viewport.width);await context.close();
 }
}finally{await writeFile(root+'/run-header-browser-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
