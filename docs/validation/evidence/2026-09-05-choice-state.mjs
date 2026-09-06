import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
const root='/var/tmp/noema-suite-run-20260905',r=JSON.parse(await readFile(root+'/chat-choice-free-text-results.json','utf8'));
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();await p.goto('https://noema.kevinpei.com');await p.getByRole('textbox',{name:'Message',exact:true}).waitFor();
const q=async(query,variables)=>await p.evaluate(async({query,variables})=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json(),{query,variables});
const read=async()=>{const data=await q('{primaryConversation{latestTranscriptPage(limit:200){items{itemId turnId item{__typename ... on MultipleChoiceSelection{promptItemId selectedOptions{id label}} ... on AssistantText{text} ... on Activity{title status}}}}}}');return data.data.primaryConversation.latestTranscriptPage.items.filter(i=>i.turnId===r.prompt.turnId);};
const before=await read();console.log(JSON.stringify({before}));
if(!before.some(i=>i.item.__typename==='MultipleChoiceSelection')){console.log(JSON.stringify(await q('mutation($input:SendMultipleChoiceSelectionInput!){sendMultipleChoiceSelection(input:$input){conversationId}}',{input:{conversationId:r.conversationId,promptItemId:r.prompt.itemId,selectedOptionIds:['beta'],clientMessageId:'audit-free-text-cleanup'}})));}
await writeFile(root+'/choice-state-results.json',JSON.stringify({before,after:await read()},null,2)+'\n');
}finally{await b.close();}
