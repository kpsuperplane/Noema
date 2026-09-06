import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:origin,provider:'codex',pages:[]};
let armed=false,streaming=false,firstDelta;const started=new Promise(resolve=>firstDelta=resolve);
try{
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),page=await context.newPage();
 page.on('websocket',ws=>ws.on('framereceived',frame=>{const m=JSON.parse(String(frame.payload)),e=m.payload?.data?.conversationEvents;if(!armed)return;if(e?.__typename==='AssistantTextDeltaEvent'){streaming=true;firstDelta();}if(e?.__typename==='TurnCompletedEvent')streaming=false;}));
 await page.goto(origin);await page.getByRole('textbox',{name:'Message',exact:true}).waitFor();
 const gql=async(query,variables)=>{const r=await page.evaluate(async({query,variables})=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json(),{query,variables});assert.equal(r.errors,undefined);return r.data;};
 const conversationId=(await gql('{primaryConversation{conversationId}}')).primaryConversation.conversationId;
 const query='query($input:ConversationTranscriptPageInput!){conversationTranscriptPage(input:$input){items{itemId cursor turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on Activity{title status}}}pageInfo{beforeCursor hasMoreBefore}}}';
 const read=async(cursor,limit)=>(await gql(query,{input:{conversationId,cursor,limit}})).conversationTranscriptPage;
 const baseline=await read(null,100),head=await read(null,10);assert.ok(baseline.items.length>=50);assert.deepEqual(head.items,baseline.items.slice(-10));evidence.baselineIds=baseline.items.map(i=>i.itemId);evidence.baselineHash=createHash('sha256').update(JSON.stringify(baseline.items)).digest('hex');
 const prompt='Migration audit CHAT-05. In this Chat, produce 80 numbered lines. Each line must say its number followed by: Pagination audit preserves older records while this response streams. Do not use tools or create Tasks. End with AUDIT_PAGINATION_WHEEL_44.';
 armed=true;await page.getByRole('textbox',{name:'Message',exact:true}).fill(prompt);await page.getByRole('textbox',{name:'Message',exact:true}).press('Enter');
 let timer;await Promise.race([started,new Promise((_,reject)=>timer=setTimeout(()=>reject(Error('No text stream started')),180000))]);clearTimeout(timer);assert.ok(streaming);
 const olderResponse=page.waitForResponse(r=>r.request().postData()&&JSON.parse(r.request().postData()).operationName==='ConversationTranscriptPage'&&Boolean(JSON.parse(r.request().postData()).variables?.input?.cursor),{timeout:30000});
 await page.getByRole('log').hover();await page.mouse.wheel(0,-20000);await page.getByRole('log').evaluate(el=>el.scrollTop=0);
 let cursor=head.pageInfo.beforeCursor,offset=baseline.items.length-10;const joined=[...head.items];
 for(let index=0;index<3;index++){const activeAtRead=streaming,p=await read(cursor,10);assert.deepEqual(p.items,baseline.items.slice(offset-10,offset));evidence.pages.push({index,activeAtRead,ids:p.items.map(i=>i.itemId),contentHash:createHash('sha256').update(JSON.stringify(p.items)).digest('hex')});joined.unshift(...p.items);offset-=10;cursor=p.pageInfo.beforeCursor;}
 assert.deepEqual(joined,baseline.items.slice(-40));assert.equal(new Set(joined.map(i=>i.itemId)).size,joined.length);assert.ok(evidence.pages.some(p=>p.activeAtRead));
 const browserResponse=await olderResponse;evidence.browserOperation=JSON.parse(browserResponse.request().postData()).operationName;const older=await browserResponse.json();assert.equal(older.errors,undefined);evidence.browserOlderPageLoaded=true;
 await page.getByRole('log').evaluate(el=>el.scrollTop=el.scrollHeight);await page.locator('[data-lane="assistant"]').filter({hasText:'AUDIT_PAGINATION_WHEEL_44'}).last().waitFor({timeout:180000});await page.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:180000});
 const stable=await read(head.pageInfo.beforeCursor,30);assert.deepEqual(stable.items,baseline.items.slice(-40,-10));await page.reload();await page.getByRole('textbox',{name:'Message',exact:true}).waitFor();assert.deepEqual((await read(head.pageInfo.beforeCursor,30)).items,stable.items);evidence.result='pass';console.log('PASS adjacent pages, live history load, and reload stability');
}finally{await writeFile(root+'/chat-pagination-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
