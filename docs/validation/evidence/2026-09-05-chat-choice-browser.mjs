import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905',origin='https://noema.kevinpei.com';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const evidence={instance:origin,provider:'codex'};
try{
 const options={serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'};
 const a=await browser.newContext(options),b=await browser.newContext(options),page=await a.newPage(),other=await b.newPage();await page.goto(origin);await other.goto(origin);
 const gql=async(p,query,variables)=>await p.evaluate(async({query,variables})=>(await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json()),{query,variables});
 const read=async()=>{const r=await gql(page,'{primaryConversation{conversationId latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on UserText{text} ... on AssistantText{text} ... on MultipleChoicePrompt{prompt selectionMode options{id label}} ... on MultipleChoiceSelection{promptItemId selectedOptions{id label}} ... on Activity{title status metadata}}}}}}');assert.equal(r.errors,undefined);return r.data.primaryConversation;};
 const title='Migration audit choice — café 日本語 🧭';
 const prompt='Migration audit CHAT-09/10. Use noema.present_multiple_choice to ask exactly '+JSON.stringify(title)+'. Use pick_one with options [{"id":"alpha","label":"Audit Alpha"},{"id":"beta","label":"Audit Beta"}]. Wait for my selection. After selection, reply with AUDIT_CHOICE_SELECTED followed by the selected option label. Do not create Tasks or use external services.';
 if(process.env.AUDIT_RESUME!=='1'){await page.getByRole('textbox',{name:'Message',exact:true}).fill(prompt);await page.getByRole('textbox',{name:'Message',exact:true}).press('Enter');}
 await page.getByRole('radio',{name:'Audit Alpha',exact:true}).waitFor({timeout:180000});await other.getByRole('radio',{name:'Audit Alpha',exact:true}).waitFor({timeout:180000});
 const c=await read(),matches=c.latestTranscriptPage.items.filter(i=>i.item.__typename==='MultipleChoicePrompt'&&i.item.prompt===title);assert.equal(matches.length,1);const question=matches[0];evidence.conversationId=c.conversationId;evidence.prompt=question;await writeFile(root+'/chat-choice-browser-results.json',JSON.stringify(evidence,null,2)+'\n');
 assert.deepEqual(question.item.options,[{id:'alpha',label:'Audit Alpha'},{id:'beta',label:'Audit Beta'}]);
 await page.getByRole('radio',{name:'Audit Beta',exact:true}).check();await page.locator('[data-lane="assistant"]').filter({hasText:'AUDIT_CHOICE_SELECTED'}).last().waitFor({timeout:180000});await page.getByRole('button',{name:'Send message',exact:true}).waitFor({timeout:180000});
 const after=await read(),selection=after.latestTranscriptPage.items.filter(i=>i.item.__typename==='MultipleChoiceSelection'&&i.item.promptItemId===question.itemId);assert.equal(selection.length,1);assert.deepEqual(selection[0].item.selectedOptions,[{id:'beta',label:'Audit Beta'}]);
 const promptTurn=after.latestTranscriptPage.items.filter(i=>i.turnId===question.turnId);const answer=after.latestTranscriptPage.items.find(i=>i.item.__typename==='AssistantText'&&i.item.text.includes('AUDIT_CHOICE_SELECTED')&&i.item.text.includes('Audit Beta'));assert.ok(answer);assert.notEqual(answer.turnId,question.turnId);
 const stale=await gql(other,'mutation($input:SendMultipleChoiceSelectionInput!){sendMultipleChoiceSelection(input:$input){conversationId}}',{input:{conversationId:c.conversationId,promptItemId:question.itemId,selectedOptionIds:['alpha'],clientMessageId:'audit-stale-choice-20260905'}});assert.ok(stale.errors?.length,'The stale answer was accepted');evidence.staleErrors=stale.errors.map(e=>e.message);
 assert.deepEqual((await read()).latestTranscriptPage.items.filter(i=>i.turnId===question.turnId),promptTurn);assert.ok(await other.getByRole('radio',{name:'Audit Alpha',exact:true}).isDisabled());
 await page.reload();await page.getByRole('radio',{name:'Audit Beta',exact:true}).waitFor();assert.ok(await page.getByRole('radio',{name:'Audit Beta',exact:true}).isChecked());assert.ok(await page.getByRole('radio',{name:'Audit Beta',exact:true}).isDisabled());assert.deepEqual((await read()).latestTranscriptPage.items.filter(i=>i.turnId===question.turnId),promptTurn);
 evidence.promptTurn=promptTurn;evidence.answer=answer;evidence.selection='pass';evidence.staleSubmission='pass';console.log('PASS single choice and stale submission');
}finally{await writeFile(root+'/chat-choice-browser-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
