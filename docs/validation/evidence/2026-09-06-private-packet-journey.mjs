import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
import {startPacketSink} from './2026-09-06-packet-sink.mjs';
const origin='https://noema.kevinpei.com',sink=await startPacketSink(),evidence={instance:origin,syntheticPrivateSource:true};
const persist=()=>writeFile('docs/validation/evidence/2026-09-06-private-packet-journey-results.json',JSON.stringify(evidence,null,2)+'\n');
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});let gql,server;
try{
 const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();await p.goto(origin);
 gql=(query,variables={})=>p.evaluate(async({query,variables})=>{const body=await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json();if(body.errors)throw new Error(JSON.stringify(body.errors));return body.data;},{query,variables});
 server=(await gql('mutation($input:CreateMcpServerInput!){createMcpServer(input:$input){server{mcpServerId connectionRevision policyRevision}}}',{input:{displayName:'Migration audit private packet sink',transportKind:'streamable_http',http:{url:sink.url}}})).createMcpServer.server;
 evidence.server=server;await persist();
 await gql('mutation($input:SaveCapabilityConnectionPolicyInput!){saveCapabilityConnectionPolicy(input:$input){status}}',{input:{kind:'MCP',connectionId:server.mcpServerId,expectedConnectionRevision:server.connectionRevision,expectedPolicyRevision:server.policyRevision,dataSharingPolicy:'allow_automatically',unsafeActionPolicy:'always_ask'}});
 const source=(await gql('mutation($input:CaptureTaskInput!){captureTask(input:$input){task{taskId taskDocument revision}}}',{input:{workspaceId:'workspace:personal',title:'Migration audit — private invoice source',taskDocument:'# Synthetic private invoice\n\nReference: INVOICE-AUDIT-42\nClient: Café 日本語 Workshop\nQuantity: 3\nUnit price: EUR 125.50\nHandling: EUR 40.00\nTax information: not supplied.\nAuthorized destination: audit-packet-box.\nDo not disclose to any other recipient.',clientMutationId:crypto.randomUUID()}})).captureTask.task;evidence.source=source;await persist();
 const marker='AUDIT_PRIVATE_PACKET_COMPLETE_20260906';
 const prompt='Migration audit JOURNEY-05. Read native Task '+source.taskId+' as the private invoice source. Use the calculation tool to compute the amount with handling. Do not invent tax information. Create and publish a concise Markdown packet Artifact preserving the exact reference, client name, quantities, unit price, handling, and calculated amount. Use the Migration audit private packet sink tool audit_upload_packet exactly once, for destination audit-packet-box, with the exact published artifact_id, artifact_version_id, and full packet_markdown. Request approval before that one upload and wait for my decision. After approval, verify the returned receipt and report its ID and Artifact identity. Do not send email, use any other external service, or execute the source Task. End with '+marker+'.';
 evidence.prompt=prompt;await p.getByRole('textbox',{name:'Message',exact:true}).fill(prompt);await p.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
 const preliminary=p.getByRole('radio',{name:'Approve one upload to audit-packet-box',exact:true}).and(p.locator(':enabled'));
 const approval=p.getByRole('button',{name:'Approve once',exact:true});
 const completed=p.locator('[data-lane="assistant"]').filter({hasText:marker}).last();
 const first=await Promise.race([preliminary.waitFor({timeout:240000}).then(()=> 'choice'),approval.waitFor({timeout:240000}).then(()=> 'action'),completed.waitFor({timeout:240000}).then(()=> 'text')]);
 if(first==='choice'){await preliminary.check();evidence.preliminaryConfirmation=true;await persist();}
 if(first==='text'){evidence.preliminaryAnswer=await completed.innerText();await p.getByRole('button',{name:'Send message',exact:true}).waitFor();await p.getByRole('textbox',{name:'Message',exact:true}).fill('I approve preparation and exactly one upload of this synthetic invoice packet to audit-packet-box. Publish the Artifact and call the connected audit_upload_packet tool. Let the configured action policy request its exact approval. Do not ask another preliminary question. Continue JOURNEY-05 and end with '+marker+'.');await p.getByRole('textbox',{name:'Message',exact:true}).press('Enter');}
 await approval.waitFor({timeout:240000});
 const pending=(await gql('{pendingHumanInterventions{__typename ... on GovernedAction{actionId revision capabilityName arguments}}}')).pendingHumanInterventions;
 const action=pending.find(action=>action.__typename==='GovernedAction'&&action.capabilityName.includes(server.mcpServerId)&&action.capabilityName.endsWith('audit_upload_packet'));
 assert.ok(action);evidence.approval=action;await persist();
 assert.equal(action.arguments.destination,'audit-packet-box');assert.ok(action.arguments.packet_markdown.includes('INVOICE-AUDIT-42')&&action.arguments.packet_markdown.includes('Café 日本語 Workshop')&&action.arguments.packet_markdown.includes('416.50'));assert.equal(sink.receipts.length,0);
 const version=action.arguments.artifact_version_id.replace(/^artifact_version:/,'');
 const download=await c.request.get(origin+'/artifacts/versions/'+version+'/download');assert.equal(download.status(),200);assert.equal(await download.text(),action.arguments.packet_markdown);evidence.approvedBytesMatchArtifact=true;
 await p.getByRole('button',{name:'Approve once',exact:true}).click();
 await p.locator('[data-lane="assistant"]').filter({hasText:marker}).last().waitFor({timeout:240000});await p.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:240000});
 assert.equal(sink.receipts.length,1);evidence.receipt=sink.receipts[0];assert.equal(evidence.receipt.packet_markdown,action.arguments.packet_markdown);assert.equal(evidence.receipt.artifact_id,action.arguments.artifact_id);assert.equal(evidence.receipt.artifact_version_id,action.arguments.artifact_version_id);
 const items=(await gql('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{metadata}}}}}}')).primaryConversation.latestTranscriptPage.items;
 const user=items.find(item=>item.item.__typename==='UserText'&&item.item.text===prompt);assert.ok(user);evidence.turnId=user.turnId;const turn=items.filter(item=>item.turnId===user.turnId);
 evidence.answer=turn.filter(item=>item.item.__typename==='AssistantText').map(item=>item.item.text).join('\n');evidence.tools=turn.filter(item=>item.item.__typename==='Activity'&&item.item.metadata.action?.call_id).map(item=>({name:item.item.metadata.action.name,success:item.item.metadata.action.success}));
 evidence.sourceAfter=(await gql('query($id:String!){task(taskId:$id){taskId stage{key} revision}}',{id:source.taskId})).task;assert.equal(evidence.sourceAfter.stage.key,'inbox');assert.equal(evidence.sourceAfter.revision,source.revision);
}catch(error){evidence.error=String(error);throw error;}
finally{if(server&&gql){try{evidence.integrationRemoved=(await gql('mutation($id:String!){deleteMcpServer(mcpServerId:$id)}',{id:server.mcpServerId})).deleteMcpServer;}catch(error){evidence.cleanupError=String(error);}}evidence.uploadCount=sink.receipts.length;await persist();await b.close();await sink.close();}
