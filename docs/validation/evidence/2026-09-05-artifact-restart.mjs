import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',phase=process.argv[2];
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();await page.goto('https://noema.kevinpei.com');
 let a;
 const bytes=Buffer.alloc(40*1024);for(let i=0;i<bytes.length;i++)bytes[i]=i%251;
 if(phase==='before'){
  const task=JSON.parse(await readFile(root+'/public-artifact-results.json','utf8')).task;
  const r=await page.evaluate(async({task,base64})=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'mutation($input:CreateTaskLocalArtifactInput!){createTaskLocalArtifact(input:$input){artifactId currentVersion{artifactVersionId downloadUrl byteSize mediaType}}}',variables:{input:{taskId:task.taskId,expectedRevision:task.revision,expectedGeneration:task.generation,title:'Migration audit maximum upload',filename:'maximum-upload.bin',mediaType:'application/octet-stream',contentBase64:base64}}})})).json()),{task,base64:bytes.toString('base64')});
  assert.equal(r.errors,undefined);a=r.data.createTaskLocalArtifact;await writeFile(root+'/artifact-restart-source.json',JSON.stringify(a,null,2)+'\n');
 }else{assert.equal(phase,'after');a=JSON.parse(await readFile(root+'/artifact-restart-source.json','utf8'));}
 const r=await page.evaluate(async url=>{const r=await fetch(url);return {status:r.status,headers:Object.fromEntries(r.headers),bytes:Array.from(new Uint8Array(await r.arrayBuffer()))};},a.currentVersion.downloadUrl);
 assert.equal(r.status,200);assert.deepEqual(Buffer.from(r.bytes),bytes);assert.equal(Number(r.headers['content-length']),bytes.length);assert.equal(r.headers['content-type'],'application/octet-stream');assert.ok(r.headers['content-disposition'].includes('maximum-upload.bin'));assert.equal(r.headers['cache-control'],'no-store');
 const evidence={instance:'https://noema.kevinpei.com',phase,result:'pass',artifactId:a.artifactId,versionId:a.currentVersion.artifactVersionId,size:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex'),detail:'The largest supported Task upload downloads with exact bytes, filename, media type, length, and private cache policy.'};
 await writeFile(root+'/artifact-restart-'+phase+'.json',JSON.stringify(evidence,null,2)+'\n');console.log('PASS',phase);
}finally{await browser.close();}
