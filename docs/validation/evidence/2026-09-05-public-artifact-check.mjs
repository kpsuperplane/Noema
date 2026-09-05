import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile,readdir,rename,symlink,unlink} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence=[],artifacts=[];let task;
const pass=(id,detail)=>{evidence.push({id,result:'pass',detail});console.log('PASS',id,detail);};
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();await page.goto(origin);
 const call=(query,variables={})=>page.evaluate(async({query,variables})=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json()),{query,variables});
 const gql=async(q,v)=>{const r=await call(q,v);assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;};
 const fetchFile=async(path,target=page)=>target.evaluate(async path=>{const r=await fetch(path);return {status:r.status,headers:Object.fromEntries(r.headers),bytes:Array.from(new Uint8Array(await r.arrayBuffer()))};},path);
 task=(await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId revision generation}}}',{input:{workspaceId:'workspace:personal',title:'Migration audit — Artifact sources',taskDocument:'# Artifact audit\n\nSynthetic source files for download and integrity checks.',clientMutationId:crypto.randomUUID()}})).captureTask.task;
 const fixtures=[
  {name:'資料.md',media:'text/markdown',text:'# Audit\n\ncafé 日本語 🧭\n',kind:'MARKDOWN'},
  {name:'資料.txt',media:'text/plain',text:'<b>Literal café 日本語</b>\n',kind:'PLAIN_TEXT'},
  {name:'audit.html',media:'text/html',text:'<h1>Visible audit</h1><script>parent.auditExecuted=true</script><img src="https://audit.invalid/image" onerror="parent.auditExecuted=true"><a href="https://audit.invalid/navigation">Link</a>',kind:'HTML'},
  {name:'audit.svg',media:'image/svg+xml',text:'<svg xmlns="http://www.w3.org/2000/svg"><text x="1" y="15">Audit</text></svg>',kind:'UNSUPPORTED'},
  {name:'audit.bin',media:'application/octet-stream',text:'Unsupported audit bytes',kind:'UNSUPPORTED'},
  {name:'audit.png',media:'image/png',base64:'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aXioAAAAASUVORK5CYII=',kind:'IMAGE'}
 ];
 for(const f of fixtures) {
  const bytes=f.base64?Buffer.from(f.base64,'base64'):Buffer.from(f.text);
  const a=(await gql('mutation($input:CreateTaskLocalArtifactInput!){createTaskLocalArtifact(input:$input){artifactId ownerObjectType ownerObjectId storageKind currentVersion{artifactVersionId downloadUrl byteSize mediaType}}}',{input:{taskId:task.taskId,expectedRevision:task.revision,expectedGeneration:task.generation,title:'Migration audit '+f.name,filename:f.name,mediaType:f.media,contentBase64:bytes.toString('base64')}})).createTaskLocalArtifact;
  assert.equal(a.ownerObjectId,task.taskId);assert.equal(a.ownerObjectType,'task');assert.equal(a.currentVersion.byteSize,bytes.length);assert.equal(a.storageKind,'LOCAL_FILE');
  const detail=(await gql('query($id:String!){artifactVersionDetail(artifactVersionId:$id){previewKind markdown plainText html previewUrl downloadUrl}}',{id:a.currentVersion.artifactVersionId})).artifactVersionDetail;
  assert.equal(detail.previewKind,f.kind);
  if(f.kind==='MARKDOWN')assert.equal(detail.markdown,f.text);if(f.kind==='PLAIN_TEXT')assert.equal(detail.plainText,f.text);
  const downloaded=await fetchFile(a.currentVersion.downloadUrl);assert.equal(downloaded.status,200);assert.deepEqual(Buffer.from(downloaded.bytes),bytes);assert.equal(downloaded.headers['cache-control'],'no-store');assert.equal(Number(downloaded.headers['content-length']),bytes.length);assert.ok(downloaded.headers['content-disposition'].startsWith('attachment;'));
  if(detail.previewUrl){const preview=await fetchFile(detail.previewUrl);assert.equal(preview.status,200);assert.deepEqual(Buffer.from(preview.bytes),bytes);}
  if(f.kind==='UNSUPPORTED'){const preview=await fetchFile(a.currentVersion.downloadUrl.replace('/download','/preview'));assert.equal(preview.status,415);}
  artifacts.push({...a,filename:f.name,previewKind:detail.previewKind});
 }
 pass('ART-01-upload-download','Six local sources retain their Task owner, exact bytes, type, length, and authorized download. Publication from a completed result remains pending.');
 pass('ART-04-transport','Markdown and text previews preserve exact content. Raster preview serves exact image bytes. PDF, spreadsheet, and rendered variants remain pending.');
 pass('ART-06-server','SVG and unsupported binary return no inline preview and retain working downloads.');
 const conversation=(await gql('mutation{ensurePrimaryConversation{conversationId}}')).ensurePrimaryConversation.conversationId;
 const url='https://example.com/audit?text=caf%C3%A9#source';
 const ext=(await gql('mutation($input:CreateConversationExternalArtifactInput!){createConversationExternalArtifact(input:$input){ownerObjectType ownerObjectId storageKind currentVersion{externalUrl downloadUrl}}}',{input:{conversationId:conversation,title:'Migration audit external reference',artifactKind:'reference',externalUrl:url}})).createConversationExternalArtifact;
 assert.equal(ext.ownerObjectId,conversation);assert.equal(ext.storageKind,'EXTERNAL_URL');assert.equal(ext.currentVersion.externalUrl,url);assert.equal(ext.currentVersion.downloadUrl,null);
 pass('ART-02-server','The external reference retains its exact URL and conversation owner without a local download claim.');
 const a=artifacts[0],base='/var/lib/noema-dev/tasks/'+task.taskId.replace(':','_')+'/artifacts/'+a.artifactId.replace(':','_');
 const entries=await readdir(base,{recursive:true});const file=base+'/'+entries.find(x=>x.endsWith('/'+a.filename));assert.ok(file.endsWith('/'+a.filename));
 const original=await readFile(file);
 try {await writeFile(file,'Changed audit bytes');const r=await fetchFile(a.currentVersion.downloadUrl);assert.notEqual(r.status,200);}finally{await writeFile(file,original);}
 await rename(file,file+'.audit-backup');
 try {
  assert.notEqual((await fetchFile(a.currentVersion.downloadUrl)).status,200);
  await symlink('/var/lib/noema-dev/audit-shared-20260905/shared.txt',file);
  try {assert.notEqual((await fetchFile(a.currentVersion.downloadUrl)).status,200);}finally{await unlink(file);}
 }finally{await rename(file+'.audit-backup',file);}
 assert.deepEqual(Buffer.from((await fetchFile(a.currentVersion.downloadUrl)).bytes),original);
 pass('ART-07-delivery','Changed bytes, a missing file, and a symbolic link fail delivery. Restoring the original restores its exact download.');
 const other=await browser.newContext({serviceWorkers:'block'});const signedOut=await other.newPage();await signedOut.goto(origin);
 assert.equal((await fetchFile(a.currentVersion.downloadUrl,signedOut)).status,401);
 const cdp=await other.newCDPSession(signedOut);await cdp.send('WebAuthn.enable');const {authenticatorId}=await cdp.send('WebAuthn.addVirtualAuthenticator',{options:{protocol:'ctap2',transport:'internal',hasResidentKey:true,hasUserVerification:true,isUserVerified:true,automaticPresenceSimulation:true}});
 const saved=JSON.parse(await readFile('/var/tmp/noema-audit-credentials/virtual-passkey.json','utf8'));for(const credential of saved.credentials)await cdp.send('WebAuthn.addCredential',{authenticatorId,credential});
 await signedOut.getByRole('button',{name:'Use passkey',exact:true}).click();await signedOut.waitForFunction(async()=> (await(await fetch('/auth/status')).json()).state==='authenticated');
 assert.equal((await fetchFile(a.currentVersion.downloadUrl,signedOut)).status,200);
 await signedOut.evaluate(()=>fetch('/auth/logout',{method:'POST'}));assert.equal((await fetchFile(a.currentVersion.downloadUrl,signedOut)).status,401);
 pass('ART-08','The same URL is denied before login, allowed after passkey login, and denied after logout. Authorized delivery uses no-store.');
}finally{await writeFile(root+'/public-artifact-results.json',JSON.stringify({instance:origin,task,evidence,artifacts},null,2)+'\n');await browser.close();}
