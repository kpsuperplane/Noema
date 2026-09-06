import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:origin,provider:'codex',transport:'Direct HTTPS origin',cases:[]};
let armed=false,cut=false,connections=0,deltaCount=0;
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});
 await context.routeWebSocket('**/graphql/ws',ws=>{
  connections++;const server=ws.connectToServer();
  server.onMessage(message=>{
   const value=JSON.parse(String(message)),event=value.payload?.data?.conversationEvents;
   if(armed&&event?.__typename==='AssistantTextDeltaEvent'){
    deltaCount++;
    if(!cut){cut=true;evidence.disconnectedTurn=event.turnId;evidence.droppedDelta=true;void server.close({code:1012,reason:'Controlled audit disconnect'});void ws.close({code:1012,reason:'Controlled audit disconnect'});return;}
   }
   ws.send(message);
  });
 });
 const page=await context.newPage();await page.goto(origin);
 const gql=async(query,variables)=>{const r=await page.evaluate(async({query,variables})=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json()),{query,variables});assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;};
 const read=async()=> (await gql('{primaryConversation{conversationId latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{title status} ... on TaskReference{taskId}}}}}}')).primaryConversation;
 const tasks=async()=> (await gql('{tasks(input:{workspaceId:"workspace:personal",scope:ALL},first:100){edges{node{taskId}}}}')).tasks.edges.map(e=>e.node.taskId).sort();
 const send=async(prompt,end)=>{await page.getByRole('textbox',{name:'Message',exact:true}).fill(prompt);await page.getByRole('textbox',{name:'Message',exact:true}).press('Enter');await page.locator('[data-lane="assistant"]').filter({hasText:end}).last().waitFor({timeout:180000});await page.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:180000});};
 const simple='Migration audit CHAT-01 persistence. Reply with exactly AUDIT_CHAT_SIMPLE_42 and no other text.';
 await send(simple,'AUDIT_CHAT_SIMPLE_42');
 let c=await read();const getTurn=(c,prompt)=>{const u=c.latestTranscriptPage.items.filter(i=>i.item.__typename==='UserText'&&i.item.text===prompt);assert.equal(u.length,1);return c.latestTranscriptPage.items.filter(i=>i.turnId===u[0].turnId);};
 const initial=getTurn(c,simple);assert.equal(initial.filter(i=>i.item.__typename==='AssistantText').length,1);assert.equal(initial.find(i=>i.item.__typename==='AssistantText').item.text,'AUDIT_CHAT_SIMPLE_42');assert.equal(initial[0].item.__typename,'UserText');
 await page.reload();await page.locator('[data-lane="assistant"]').filter({hasText:'AUDIT_CHAT_SIMPLE_42'}).last().waitFor();assert.deepEqual(getTurn(await read(),simple),initial);evidence.cases.push({id:'CHAT-01',result:'pass',turn:initial});
 const exact='# Reconnect audit\n\nCafé 日本語 🧭 remain intact.\n\n- First audit item\n- Second audit item\n\n```js\nconst audit = "café 日本語 🧭";\nconsole.log(audit);\n```\n\n[Audit link](https://example.com/noema-audit?q=42)\n\n'+Array.from({length:15},(_,i)=>`Audit line ${i+1}: preserve order and exact content.`).join('\n\n')+'\n\nAUDIT_RECONNECT_COMPLETE_42';
 const prompt='Migration audit CHAT-03/06/08. Complete this response here in foreground Chat. Do not create or run any Task. Do not use tools or access external services. Return only the exact Markdown string from this JSON value, without enclosing fences:\n'+JSON.stringify({markdown:exact});
 const taskIds=await tasks();armed=true;await send(prompt,'AUDIT_RECONNECT_COMPLETE_42');armed=false;
 assert.ok(cut,'The controlled socket disconnect did not occur during a text delta');assert.ok(connections>=2,'The browser did not reconnect');
 c=await read();const turn=getTurn(c,prompt),answers=turn.filter(i=>i.item.__typename==='AssistantText');assert.equal(answers.length,1);assert.equal(answers[0].item.text.trim(),exact);assert.equal(turn[0].item.__typename,'UserText');assert.equal(turn.filter(i=>i.item.__typename==='TaskReference').length,0);assert.deepEqual(await tasks(),taskIds);
 const assistant=page.locator('[data-lane="assistant"]').filter({hasText:'AUDIT_RECONNECT_COMPLETE_42'}).last();
 await assistant.getByRole('heading',{name:'Reconnect audit',exact:true}).waitFor();assert.equal(await assistant.locator('pre code').innerText(),'const audit = "café 日本語 🧭";\nconsole.log(audit);');assert.equal(await assistant.getByRole('link',{name:'Audit link',exact:true}).getAttribute('href'),'https://example.com/noema-audit?q=42');assert.equal(await assistant.locator('li').count(),2);
 await page.reload();await page.getByRole('heading',{name:'Reconnect audit',exact:true}).waitFor();assert.deepEqual(getTurn(await read(),prompt),turn);
 const other=await browser.newContext({viewport:{width:390,height:900},serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const phone=await other.newPage();await phone.goto(origin);await phone.getByRole('heading',{name:'Reconnect audit',exact:true}).waitFor();
 const phoneText=await phone.locator('[data-lane="assistant"]').filter({hasText:'AUDIT_RECONNECT_COMPLETE_42'}).last().innerText();assert.ok(phoneText.includes('Café 日本語 🧭 remain intact.'));assert.ok(phoneText.includes('Audit line 15: preserve order and exact content.'));
 evidence.cases.push({id:'CHAT-03',result:'pass',scope:'Live Codex text streaming; tool-result disconnect remains pending.',connections,deltaCount,turn},{id:'CHAT-06',result:'pass',scope:'Two Chromium contexts, including phone width; native clients remain pending.'},{id:'CHAT-08',result:'pass',scope:'Foreground text request creates no Task.'});
 console.log('PASS CHAT-01, text reconnect, rich text, and foreground execution');
}finally{await writeFile(root+'/chat-reconnect-browser-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
