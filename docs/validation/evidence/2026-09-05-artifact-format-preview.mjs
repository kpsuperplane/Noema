import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',published=JSON.parse(await readFile(root+'/artifact-format-publish-results.json','utf8'));
const fixtures=JSON.parse(await readFile(root+'/artifact-format-fixtures.json','utf8'));
const pdfOnly=process.env.AUDIT_PDF_ONLY==='1';
const browser=await chromium.launch({channel:process.env.AUDIT_BROWSER||undefined,headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});const evidence=[];
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();
 await page.addInitScript(()=>{window.auditFontViolations=[];document.addEventListener('securitypolicyviolation',e=>{if(e.effectiveDirective==='font-src')window.auditFontViolations.push(e.effectiveDirective);});});
 await page.goto(published.route);
 if(pdfOnly){const fonts=await page.evaluate(async()=>{const loaded=await document.fonts.load('16px KaTeX_Size3');return {loaded:loaded.length,ready:document.fonts.check('16px KaTeX_Size3'),violations:window.auditFontViolations};});assert.equal(fonts.loaded,1);assert.equal(fonts.ready,true);assert.deepEqual(fonts.violations,[]);evidence.push({id:'font-policy',result:'pass',detail:'The bundled KaTeX_Size3 font loads through the live response policy without a font CSP violation.'});}
 const data=await page.evaluate(async id=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($id:String!){artifacts(ownerObjectType:"task",ownerObjectId:$id){artifactId title currentVersion{artifactVersionId downloadUrl}}}',variables:{id}})})).json()),published.taskId);
 assert.equal(data.errors,undefined);assert.equal(data.data.artifacts.length,fixtures.length);
 for(const f of fixtures.filter(f=>!pdfOnly||f.filename==='preview.pdf')){
  const a=data.data.artifacts.find(a=>a.title===f.title);assert.ok(a);
  const bytes=await page.evaluate(async url=>await(await fetch(url)).text(),a.currentVersion.downloadUrl);assert.equal(bytes,f.content);
  await page.goto(published.route);await page.getByRole('button',{name:'Transcript',exact:true}).click();await page.getByRole('region',{name:'Transcript',exact:true}).hover();
  const card=page.getByRole('button',{name:new RegExp('^'+f.title+' Report')});
  for(let n=0;n<40&&!await card.count();n++){await page.mouse.wheel(0,-500);await page.waitForTimeout(200);}
  await card.click();
  if(f.filename==='fallback.svg'||f.filename==='fallback.bin'){
   await page.getByText('Preview unavailable',{exact:true}).waitFor();assert.equal(await page.locator('iframe').count(),0);
   const link=page.getByRole('link',{name:'Download',exact:true});assert.equal(await link.getAttribute('href'),a.currentVersion.downloadUrl);
   const [d]=await Promise.all([page.waitForEvent('download'),link.click()]);assert.equal(d.suggestedFilename(),f.filename);assert.equal((await readFile(await d.path())).toString(),f.content);
  }else if(f.filename==='literal.txt'){
   await page.locator('pre').filter({hasText:'Literal café 日本語 🧭'}).waitFor();assert.equal(await page.locator('pre').textContent(),f.content);assert.equal(await page.locator('pre b').count(),0);
  }else{
   const iframe=page.locator('iframe[title="Audit PDF preview preview"]');await iframe.waitFor();
   assert.equal(await iframe.getAttribute('src'),a.currentVersion.downloadUrl.replace('/download','/preview'));
   await page.waitForTimeout(1500);await page.screenshot({path:root+'/artifact-pdf-'+(process.env.AUDIT_BROWSER||'shell')+'.png',fullPage:true});
  }
  evidence.push({filename:f.filename,result:'pass',detail:f.filename==='preview.pdf'?'Exact published PDF bytes and the authorized iframe source match. Native viewer readability needs visual review.':'The production card opens the expected text preview or download-only fallback. Downloaded bytes remain exact.'});console.log('PASS',f.filename);
 }
}finally{await writeFile(root+(pdfOnly?'/artifact-pdf-browser-results.json':'/artifact-format-preview-results.json'),JSON.stringify({instance:'https://noema.kevinpei.com',taskId:published.taskId,evidence},null,2)+'\n');await browser.close();}
