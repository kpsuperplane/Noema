import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile,readdir,rename,symlink,unlink} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',published=JSON.parse(await readFile(root+'/public-publish-results.json','utf8'));
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence=[];let file,original;
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();
 await page.goto(published.route);
 const gql=async(query,variables)=>{const r=await page.evaluate(async({query,variables})=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json()),{query,variables});assert.equal(r.errors,undefined);return r.data;};
 const artifacts=(await gql('query($id:String!){artifacts(ownerObjectType:"task",ownerObjectId:$id){artifactId title ownerObjectId storageKind currentVersion{artifactVersionId downloadUrl byteSize mediaType}}}',{id:published.taskId})).artifacts;
 const markdown=artifacts.find(a=>a.title==='Migration audit Markdown');assert.ok(markdown);assert.equal(markdown.ownerObjectId,published.taskId);assert.equal(markdown.currentVersion.mediaType,'text/markdown');
 const versionId=markdown.currentVersion.artifactVersionId;
 const resultLink=page.locator('a[href="'+markdown.currentVersion.downloadUrl+'"]').first();
 await resultLink.waitFor();
 const [downloaded]=await Promise.all([page.waitForEvent('download'),resultLink.click()]);
 assert.equal(downloaded.suggestedFilename(),'audit-report.md');const bytes=await readFile(await downloaded.path());assert.equal(bytes.toString(),'# Version two\n\nUpdated café 日本語 🧭\n');assert.equal(bytes.length,markdown.currentVersion.byteSize);
 evidence.push({id:'ART-01',result:'pass',detail:'Reopened the completed Task result and clicked its real download link. Task owner, media type, filename, size, and exact version-two bytes match.'});console.log('PASS ART-01');
 const openMarkdown=async()=>{
  await page.goto(published.route);await page.getByRole('button',{name:'Transcript',exact:true}).click();
  const region=page.getByRole('region',{name:'Transcript',exact:true});await region.hover();
  const card=page.getByRole('button',{name:'Migration audit Markdown Report · Markdown',exact:true});
  for(let n=0;n<40&&!await card.count();n++){await page.mouse.wheel(0,-500);await page.waitForTimeout(200);}
  await card.click();
 };
 await openMarkdown();await page.getByRole('heading',{name:'Version two',level:1,exact:true}).waitFor();
 const selector=page.getByRole('combobox',{name:'Artifact version',exact:true});console.log('selector',await selector.count());
 await selector.click();await page.getByRole('option',{name:'Version 1',exact:true}).click();await page.getByRole('heading',{name:'Version one',level:1,exact:true}).waitFor();
 assert.ok((await page.locator('body').innerText()).includes('Original café 日本語 🧭'));
 await selector.click();await page.getByRole('option',{name:'Version 2',exact:true}).click();await page.getByRole('heading',{name:'Version two',level:1,exact:true}).waitFor();
 evidence.push({id:'ART-03-selector',result:'pass',detail:'The browser selector displays each immutable Markdown version and returns to version two. Saved upload binding remains pending.'});console.log('PASS ART-03-selector');
 const base='/var/lib/noema-dev/tasks/'+published.taskId.replace(':','_')+'/artifacts/'+markdown.artifactId.replace(':','_');
 const entries=await readdir(base,{recursive:true});const files=entries.filter(x=>x.endsWith('/audit-report.md'));
 for(const entry of files){const path=base+'/'+entry;if((await readFile(path,'utf8'))===bytes.toString()){file=path;break;}}
 assert.ok(file);original=await readFile(file);
 const unavailable=async()=>{await openMarkdown();await page.getByRole('alert').filter({hasText:'Artifact could not load.'}).waitFor();assert.equal(await page.getByRole('heading',{name:'Version two',level:1,exact:true}).count(),0);assert.equal((await page.locator('body').innerText()).includes('Replacement audit bytes'),false);};
 try {await writeFile(file,'Replacement audit bytes');await unavailable();}finally{await writeFile(file,original);}
 await rename(file,file+'.audit-backup');
 try {
  await unavailable();await symlink('/var/lib/noema-dev/audit-shared-20260905/shared.txt',file);
  try {await unavailable();}finally{await unlink(file);}
 }finally{await rename(file+'.audit-backup',file);}
 await openMarkdown();await page.getByRole('heading',{name:'Version two',level:1,exact:true}).waitFor();
 evidence.push({id:'ART-07',result:'pass',detail:'Altered, removed, and symlinked published Markdown files show a load error without replacement content. Restoring the original restores the preview.'});console.log('PASS ART-07');
}finally{if(file&&original)await writeFile(file,original);await writeFile(root+'/artifact-followup-results.json',JSON.stringify({instance:'https://noema.kevinpei.com',taskId:published.taskId,evidence},null,2)+'\n');await browser.close();}
