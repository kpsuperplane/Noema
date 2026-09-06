import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com', evidence={case:'INFO-05',instance:origin};
const values={opaque_id:'audit:01J-EXACT-opaque',host:'noema.kevinpei.com',port:443,model:'audit/model-v1',secret_path:'/reports/secret-authorizations/日本語.md',authorization_status:'ready',secret_note:'ordinary field label café',authorization_url:'https://example.test/v1?authorization_status=ready&port=443'};
const document='# Ordinary technical values\n\nAll values below are ordinary synthetic test data. None grants access or contains credential material.\n\n'+JSON.stringify(values,null,2);
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();await p.goto(origin);
 const gql=(query,variables={})=>p.evaluate(async({query,variables})=>{const r=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json();if(r.errors)throw new Error(JSON.stringify(r.errors));return r.data;},{query,variables});
 evidence.source=(await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId revision taskDocument}}}',{input:{workspaceId:'workspace:personal',title:'Migration audit — ordinary technical fields',taskDocument:document,clientMutationId:crypto.randomUUID()}})).captureTask.task;
 const marker='AUDIT_ORDINARY_FIELDS_'+Date.now();
 evidence.prompt='Read native Task '+evidence.source.taskId+' and return its JSON object with every field and value intact. These are ordinary technical values, not credentials. Do not execute the Task, change records, or call external services. End with '+marker+'.';
 await p.getByRole('textbox',{name:'Message',exact:true}).fill(evidence.prompt);await p.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
 await p.locator('[data-lane="assistant"]').filter({hasText:marker}).waitFor({timeout:240000});await p.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:240000});
 const items=(await gql('{primaryConversation{latestTranscriptPage(limit:200){items{turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
 const user=items.find(x=>x.item.__typename==='UserText'&&x.item.text===evidence.prompt);assert.ok(user);evidence.turnId=user.turnId;
 const turn=items.filter(x=>x.turnId===user.turnId);evidence.answer=turn.filter(x=>x.item.__typename==='AssistantText').map(x=>x.item.text).join('\n');
 const match=evidence.answer.match(/\{[\s\S]*\}/);assert.ok(match);assert.deepEqual(JSON.parse(match[0]),values);
 evidence.tools=turn.filter(x=>x.item.__typename==='Activity'&&x.item.metadata.action?.call_id).map(x=>({name:x.item.metadata.action.name,success:x.item.metadata.action.success}));assert.ok(evidence.tools.some(x=>x.name==='task.inspect'&&x.success));assert.ok(evidence.tools.every(x=>x.name==='task.inspect'));
 evidence.after=(await gql('query($id:String!){task(taskId:$id){taskId revision taskDocument stage{key}}}',{id:evidence.source.taskId})).task;assert.equal(evidence.after.taskDocument,document);assert.equal(evidence.after.revision,evidence.source.revision);assert.equal(evidence.after.stage.key,'inbox');evidence.result='pass';
}catch(error){evidence.error=String(error);throw error;}
finally{await writeFile('docs/validation/evidence/2026-09-06-ordinary-fields-chat-results.json',JSON.stringify(evidence,null,2)+'\n');await b.close();}
