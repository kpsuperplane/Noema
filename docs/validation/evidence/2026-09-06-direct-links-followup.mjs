import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const published=JSON.parse(await readFile(root+'/artifact-format-publish-results.json','utf8'));
const fixture=JSON.parse(await readFile(root+'/artifact-format-fixtures.json','utf8')).find(f=>f.filename==='literal.txt');
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const results=[];
try {
 for(const width of (process.argv.includes("memory-phone")?[390]:[1440,390])) {
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});
  const page=await context.newPage();page.setDefaultTimeout(15000);
  const cases=[
   {name:'Project',path:'/tasks?project=project%3Aeb7e02135a057b326b0863dbccf19894',check:async()=>{await page.getByRole('link').filter({hasText:'PROJECT.md'}).waitFor();assert.equal(await page.getByText('Migration audit — direct execution',{exact:true}).count(),0);}},
   {name:'Memory article',path:'/memory/stargazing',check:async()=>{await page.locator('[data-slot=memory-surface] article').getByRole('heading',{name:'Stargazing',exact:true,includeHidden:true}).waitFor({state:'attached'});assert.equal(await page.locator('[data-slot=memory-surface] article').isVisible(),true);}},
   {name:'Settings',path:'/settings/tools/web',check:async()=>{await page.getByRole('combobox',{name:'Provider for web.search',exact:true}).waitFor();}}
  ];
  for(const c of cases.filter(c=>!process.argv.includes("memory-phone")||c.name==="Memory article")){await page.goto(origin+c.path);let error;try{await c.check()}catch(e){error=e.message.split('\n')[0]};results.push({width,name:c.name,path:c.path,passed:!error,...(error?{error}:{})});}
  if(process.argv.includes("memory-phone")){await context.close();continue;}
  const data=await page.evaluate(async id=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){artifacts(ownerObjectType:"task",ownerObjectId:$id){title currentVersion{downloadUrl}}}',variables:{id}})})).json()),published.taskId);
  assert.equal(data.errors,undefined);const artifact=data.data.artifacts.find(a=>a.title===fixture.title);assert.ok(artifact);
  const download=page.waitForEvent('download');await page.goto(new URL(artifact.currentVersion.downloadUrl,origin).href).catch(e=>{if(!e.message.includes('Download is starting'))throw e});
  const file=await download;assert.equal(file.suggestedFilename(),fixture.filename);assert.equal(await readFile(await file.path(),'utf8'),fixture.content);
  results.push({width,name:'Artifact direct download',path:artifact.currentVersion.downloadUrl,passed:true,exactBytes:true,filename:fixture.filename});
  await context.close();
 }
}finally{await browser.close();await writeFile(root+(process.argv.includes('memory-phone')?'/direct-memory-phone-results.json':'/direct-links-followup-results.json'),JSON.stringify({instance:origin,transport:'Direct HTTPS origin; public edge and native clients are not covered.',results},null,2)+'\n');}
console.log(JSON.stringify(results));
