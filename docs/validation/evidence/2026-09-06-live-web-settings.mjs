import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const results=[];
try {
for(const width of [1440,390]) {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});
 const page=await context.newPage();page.setDefaultTimeout(15000);
 await page.goto('https://noema.kevinpei.com/settings/tools/web');
 await page.getByRole('heading',{name:'Search',exact:true}).waitFor();
 const change=async(tool,value)=>{
  await page.getByRole('combobox',{name:`Provider for ${tool}`,exact:true}).click();
  const saved=page.waitForResponse(r=>r.url().endsWith('/graphql')&&r.request().postData()?.includes('SaveWebToolProviderBinding'));
  await page.getByRole('option',{name:value,exact:true}).click();
  const data=await (await saved).json();if(data.errors)throw Error('Provider save returned GraphQL errors');
  await page.reload();await page.getByRole('heading',{name:'Search',exact:true}).waitFor();
  if((await page.getByRole('combobox',{name:`Provider for ${tool}`,exact:true}).innerText()).trim()!==value)throw Error('Reload lost provider selection');
 };
 try {
  for(const [tool,value] of [['web.search','DuckDuckGo public search'],['web.fetch','Direct HTTP web fetch']]) {
   if((await page.getByRole('combobox',{name:`Provider for ${tool}`,exact:true}).innerText()).trim()!=='Codex')throw Error('Unexpected initial provider');
   await change(tool,value);await change(tool,'Codex');results.push({width,tool,changedTo:value,reloadPassed:true,restored:'Codex'});
  }
  await page.getByRole('button',{name:'Edit',exact:true}).click();
  await page.getByRole('dialog').waitFor();
  const dialogText=await page.getByRole('dialog').innerText();
  const screenshot=`/var/tmp/noema-suite-run-20260905/web-route-${width}.png`;
  await page.screenshot({path:screenshot,fullPage:true});
  results.push({width,routeDialog:dialogText,screenshot,focusedLabel:await page.evaluate(()=>document.activeElement?.getAttribute('aria-label'))});
  await page.getByRole('button',{name:'Cancel',exact:true}).click();
 } finally {
  for(const tool of ['web.search','web.fetch']) {
   const selector=page.getByRole('combobox',{name:`Provider for ${tool}`,exact:true});
   if(await selector.isVisible() && (await selector.innerText()).trim()!=='Codex')await change(tool,'Codex');
  }
  await context.close();
 }
}
await writeFile('/var/tmp/noema-suite-run-20260905/web-settings-live-results.json',JSON.stringify({results},null,2)+'\n');
console.log(JSON.stringify({results}));
}finally{await browser.close()}
