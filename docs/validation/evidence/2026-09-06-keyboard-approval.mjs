import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const compact=process.env.AUDIT_COMPACT==='1';
const origin='https://noema.kevinpei.com',evidence={instance:origin,scope:'Controlled approval card and mutation responses. No live decision or external effect.',clients:[]};
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 for(const width of (compact?[320]:[1440,390])) {
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:compact?480:1000}}),page=await context.newPage();
  const record={width,height:compact?480:1000,decisions:[],focus:[]};evidence.clients.push(record);
  const action={__typename:'GovernedAction',actionId:'action:audit-keyboard',revision:7,conversationId:null,taskId:null,runId:null,actionTask:null,capabilityName:'audit.upload',reviewRoute:'human',behavior:{readOnly:false,idempotent:false,destructive:false,openWorld:false},safeSummary:'Upload the synthetic audit document',target:{serviceName:'Controlled service',connectionLabel:'Audit account',serviceId:'audit',connectionId:'audit',accountId:'audit'},disclosure:{recipient:'Controlled destination',contentSummary:'Synthetic document only'},consequence:'One controlled upload request.',arguments:{document:'audit.txt'},assessment:{status:'needs_human',authorization:'needs_human',risk:'low',reasonCodes:[],explanation:'Controlled keyboard audit'},browserSessionAvailable:true,failureCode:null};
  let pending=true;
  await page.route('**/graphql',async route=>{
   const body=route.request().postDataJSON();
   if(body.operationName==='PendingHumanInterventions')return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({data:{pendingHumanInterventions:pending?[action]:[]}})});
   if(body.operationName==='ResolveGovernedAction'){
    record.decisions.push(body.variables.input);
    if(body.variables.input.decision==='APPROVE')return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({errors:[{message:'Controlled approval save failure'}],data:null})});
    pending=false;
    return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({data:{resolveGovernedAction:{...action,state:'declined',output:null}}})});
   }
   if(/\bmutation\b/.test(body.query||'')){record.blockedOperation=body.operationName;return route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({errors:[{message:'Unrelated mutation blocked by audit'}],data:null})});}
   await route.continue();
  });
  await page.goto(origin);
  const reach=async name=>{
   const target=page.getByRole('button',{name,exact:true});await target.waitFor();
   for(let step=0;step<100;step++){
    if(await target.evaluate(element=>element===document.activeElement)){
     record.focus.push(await target.evaluate(element=>({name:element.textContent,focusVisible:element.matches(':focus-visible'),outline:getComputedStyle(element).outline,visible:element.getBoundingClientRect().top>=0&&element.getBoundingClientRect().bottom<=innerHeight})));
     return;
    }
    await page.keyboard.press('Tab');
   }
   throw new Error('Keyboard cannot reach '+name);
  };
  await reach('Approve once');
  await page.screenshot({path:`/var/tmp/noema-suite-run-20260905/keyboard-approval-${width}-${compact?480:1000}.png`,animations:'disabled'});
  await page.keyboard.press('Enter');
  await page.getByText('Controlled approval save failure',{exact:false}).first().waitFor();
  await reach('Decline');await page.keyboard.press('Enter');
  await page.getByRole('button',{name:'Approve once',exact:true}).waitFor({state:'hidden'});
  assert.deepEqual(record.decisions,[{actionId:action.actionId,expectedRevision:7,decision:'APPROVE'},{actionId:action.actionId,expectedRevision:7,decision:'DECLINE'}]);
  record.cardRemovedAfterDecline=true;
  await context.close();
 }
}catch(error){evidence.error=String(error);throw error;}
finally{await writeFile(`docs/validation/evidence/2026-09-06-${compact?'approval-reflow':'keyboard-approval'}-results.json`,JSON.stringify(evidence,null,2)+'\n');await browser.close();}
