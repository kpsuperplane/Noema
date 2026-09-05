import http from 'node:http';
import {mkdir,chown,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905', folder='/var/lib/noema-dev/audit-shared-20260905';
const evidence=[];let projectId;
const call=(query,variables={})=>new Promise((resolve,reject)=>{const r=http.request({socketPath:'/tmp/noema-codex/graphql.sock',path:'/graphql',method:'POST',headers:{'content-type':'application/json'}},res=>{let text='';res.on('data',d=>text+=d);res.on('end',()=>resolve(JSON.parse(text)));});r.on('error',reject);r.end(JSON.stringify({query,variables}));});
async function gql(q,v){const r=await call(q,v);assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;}
const pass=(id,detail)=>{evidence.push({id,result:'pass',detail});console.log('PASS',id,detail);};
try {
 await mkdir(folder,{recursive:true});const metadata=await import('node:fs/promises').then(fs=>fs.stat('/var/lib/noema-dev'));await chown(folder,metadata.uid,metadata.gid);
 await writeFile(folder+'/shared.txt','Shared café 日本語 context.\n');await chown(folder+'/shared.txt',metadata.uid,metadata.gid);
 let project=(await gql('mutation($input:CreateProjectInput!){createProject(input:$input){project{projectId revision name folder}}}',{input:{workspaceId:'workspace:personal',name:'Migration audit Project',folder,clientMutationId:crypto.randomUUID()}})).createProject.project;projectId=project.projectId;
 project=(await gql('mutation($input:UpdateProjectInput!){updateProject(input:$input){project{projectId revision name folder}}}',{input:{projectId,expectedRevision:project.revision,name:'Migration audit Project café 日本語',clientMutationId:crypto.randomUUID()}})).updateProject.project;
 assert.equal(project.name,'Migration audit Project café 日本語');assert.equal(project.folder,folder);
 pass('PROJECT-01-settings','Create and rename preserve the configured shared folder. Task role reads remain pending.');
 const pd=(await gql('query($id:String!){projectDocument(projectId:$id){content digest}}',{id:projectId})).projectDocument;
 const q='mutation($input:UpdateProjectDocumentInput!){updateProjectDocument(input:$input){project{revision} document{content digest}}}';
 const original={projectId,expectedRevision:project.revision,expectedDocumentDigest:pd.digest};
 const content='# Migration audit Project\n\nShared café 日本語 🧭 context.\n';
 const saved=(await gql(q,{input:{...original,content,clientMutationId:crypto.randomUUID()}})).updateProjectDocument;
 const stale=await call(q,{input:{...original,content:'Stale audit draft',clientMutationId:crypto.randomUUID()}});assert.ok(stale.errors?.length);
 assert.equal((await gql('query($id:String!){projectDocument(projectId:$id){content}}',{id:projectId})).projectDocument.content,content);
 pass('PROJECT-02-server','A stale save fails and the exact current Project document remains readable. Browser draft retention remains pending.');
 const archived=(await gql('mutation($input:ArchiveProjectInput!){archiveProject(input:$input){project{revision archivedAt}}}',{input:{projectId,expectedRevision:saved.project.revision,clientMutationId:crypto.randomUUID()}})).archiveProject.project;assert.ok(archived.archivedAt);
 const denied=await call(q,{input:{projectId,expectedRevision:archived.revision,expectedDocumentDigest:saved.document.digest,content:'Archived edit',clientMutationId:crypto.randomUUID()}});assert.ok(denied.errors?.length);
 assert.equal((await gql('query($id:String!){projectDocument(projectId:$id){content}}',{id:projectId})).projectDocument.content,content);
 const reopened=(await gql('mutation($input:ReopenProjectInput!){reopenProject(input:$input){project{revision archivedAt}}}',{input:{projectId,expectedRevision:archived.revision,clientMutationId:crypto.randomUUID()}})).reopenProject.project;assert.equal(reopened.archivedAt,null);
 const final=(await gql(q,{input:{projectId,expectedRevision:reopened.revision,expectedDocumentDigest:saved.document.digest,content:content+'\nEditing resumed.\n',clientMutationId:crypto.randomUUID()}})).updateProjectDocument;assert.equal(final.document.content,content+'\nEditing resumed.\n');
 pass('PROJECT-04-server','Archive keeps the document readable and rejects edits. Reopen permits a new edit.');
} finally {await writeFile(root+'/public-project-results.json',JSON.stringify({instance:'https://noema.kevinpei.com',transport:'Development Unix socket for the same instance',projectId,evidence},null,2)+'\n');}
