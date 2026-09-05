import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';const published=JSON.parse(await readFile(root+'/public-publish-results.json','utf8'));
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence=[];const width=Number(process.env.AUDIT_WIDTH || 1280);
try {
 const context=await browser.newContext({viewport:{width,height:900},serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();const attempts=[];
 await context.route('https://audit.invalid/**',route=>{attempts.push(route.request().url());return route.abort();});
 await page.goto(published.route);await page.getByRole('button',{name:'Transcript',exact:true}).click();
 const transcript=page.getByRole('region',{name:'Transcript',exact:true});
 const open=async title=>{
  const card=page.getByRole('button',{name:title+' Report · Html',exact:true});
  await transcript.hover();
  for(let n=0;n<40 && !await card.count();n++){await page.mouse.wheel(0,-500);await page.waitForTimeout(200);}
  await card.waitFor();
  await card.scrollIntoViewIfNeeded();
  await card.hover();
  await page.screenshot({path:root+'/artifact-transcript-'+width+'.png',fullPage:true});
  await card.click();
 };
 await open('Migration audit HTML');const frame=page.frameLocator('iframe[title="Migration audit HTML preview"]');
 await frame.getByRole('heading',{name:'Visible audit HTML',exact:true}).waitFor();
 assert.equal(await page.locator('iframe[title="Migration audit HTML preview"]').getAttribute('sandbox'),'');
 assert.equal(await frame.locator('script,form,iframe,object,embed').count(),0);
 assert.equal(await frame.locator('[onclick],[onerror],[href],[src^="https:"]').count(),0);
 assert.equal(await page.evaluate(()=>window.auditExecuted),undefined);assert.equal(attempts.length,0);
 evidence.push({id:'ART-05',result:'pass',detail:'The actual sandboxed preview retains visible HTML and strips scripts, forms, event handlers, navigation, and external image loading.'});
 await page.waitForTimeout(500); // Let the detail-panel transition finish before visual review.
 await page.screenshot({path:root+'/artifact-preview-'+width+'.png',fullPage:true});console.log('PASS ART-05');
 const artifactData=await page.evaluate(async taskId=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){artifacts(ownerObjectType:"task",ownerObjectId:$id){artifactId title currentVersion{artifactVersionId}}}',variables:{id:taskId}})})).json()),published.taskId);
 const markdown=artifactData.data.artifacts.find(a=>a.title==='Migration audit Markdown');assert.ok(markdown);
 const versionData=await page.evaluate(async id=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){artifactVersionDetail(artifactVersionId:$id){versions{artifactVersionId versionIndex downloadUrl}}}',variables:{id}})})).json()),markdown.currentVersion.artifactVersionId);
 const versions=versionData.data.artifactVersionDetail.versions;assert.equal(versions.length,2);
 for(const v of versions){const bytes=await page.evaluate(async url=>await(await fetch(url)).text(),v.downloadUrl);assert.ok(bytes.includes(v.versionIndex===1?'Original café 日本語 🧭':'Updated café 日本語 🧭'));}
 evidence.push({id:'ART-03-versions',result:'pass',detail:'Both immutable published versions remain individually downloadable with their original distinct content. Saved upload-action binding remains pending.'});
 console.log('PASS ART-03-versions');
}finally{await writeFile(root+'/public-artifact-preview-results-'+width+'.json',JSON.stringify({instance:'https://noema.kevinpei.com',taskId:published.taskId,evidence},null,2)+'\n');await browser.close();}
