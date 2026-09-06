import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',results=[];
const claims=['Café 日本語 🧭 claim A.','Second claim B.','Repeated claim C.'],text=claims.join('\n\n');
const urls=['https://example.com/a?x=1&y=2#café','https://example.org/reference?q=日本語'];
const citations=[{title:'Source café 日本語 🧭',url:urls[0],start_index:0,end_index:claims[0].length},{title:'Other source',url:urls[1],start_index:claims[0].length+2,end_index:claims[0].length+2+claims[1].length},{title:'Repeated source title',url:urls[0],start_index:text.indexOf(claims[2]),end_index:text.length}];
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 for(const width of [1440,390]) {
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});let injected=0,faviconFailures=0;
  await context.route('**/favicons/**',r=>{faviconFailures++;return r.fulfill({status:503,contentType:'text/plain',body:'Controlled missing favicon'});});
  await context.route('**/graphql',async r=>{const body=r.request().postDataJSON();if(!body?.query?.includes('query ConversationTranscriptPage'))return r.continue();const response=await r.fetch(),data=await response.json();const items=data.data?.conversationTranscriptPage?.items;if(items){const item=[...items].reverse().find(x=>x.item?.__typename==='AssistantText');if(item){item.item.text=text;item.metadata={...item.metadata,citations};injected++;}}await r.fulfill({response,json:data});});
  const page=await context.newPage();page.setDefaultTimeout(15000);const errors=[];page.on('pageerror',e=>errors.push(e.message.split('\n')[0]));await page.goto(origin+'/');
  const message=page.locator('[data-lane="assistant"]').filter({hasText:claims[0]}).last();await message.waitFor();await message.scrollIntoViewIfNeeded();assert.ok(injected>0);
  const paragraphs=await message.evaluate((element,claims)=>claims.map(claim=>{const walker=document.createTreeWalker(element,NodeFilter.SHOW_TEXT);let node;while(node=walker.nextNode()){if(node.textContent!==claim)continue;let parent=node.parentElement;while(parent&&parent!==element&&parent.textContent===claim)parent=parent.parentElement;return parent?.textContent??null;}return null;}),claims);assert.deepEqual(paragraphs,claims.map((claim,i)=>claim+[1,2,1][i]));if(process.argv.includes('markers')){await page.screenshot({path:'/var/tmp/noema-suite-run-20260905/citation-markers-'+width+'.png',fullPage:true,animations:'disabled'});results.push({width,check:'Citation markers follow exact claims',passed:true,paragraphs});await context.close();continue;}
  const markerLinks=await message.locator('a').evaluateAll(ns=>ns.map(n=>({text:n.textContent,href:n.getAttribute('href'),parentText:n.parentElement.textContent})));
  await message.getByRole('button',{name:'Sources',exact:true}).click();const dialog=page.getByRole('dialog',{name:'Sources',exact:true});await dialog.waitFor();const sources=await dialog.locator('a').evaluateAll(ns=>ns.map(n=>({name:n.getAttribute('aria-label'),href:n.getAttribute('href')})));
  assert.deepEqual(sources,[{name:'Source 1: Source café 日本語 🧭',href:urls[0]},{name:'Source 2: Other source',href:urls[1]}]);assert.ok(faviconFailures>0);assert.deepEqual(errors,[]);
  await page.screenshot({path:'/var/tmp/noema-suite-run-20260905/citations-favicons-'+width+'.png',fullPage:true,animations:'disabled'});await page.keyboard.press('Escape');await dialog.waitFor({state:'hidden'});await page.getByRole('textbox',{name:'Message',exact:true}).waitFor();
  results.push({width,injected,faviconFailures,passed:true,markerLinks,sources,composerAvailable:true,pageErrors:errors});await context.close();
 }
}catch(e){results.push({check:'Execution stopped',error:e.message.slice(0,600)});}
finally{await browser.close();await writeFile('/var/tmp/noema-suite-run-20260905/'+(process.argv.includes('markers')?'citation-markers-results.json':'citations-favicons-results.json'),JSON.stringify({instance:origin,setup:'One browser-only synthetic transcript response. All favicon requests return 503. No server mutation.',claims,urls,results},null,2)+'\n');}
console.log(JSON.stringify(results));
