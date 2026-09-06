import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
const origin='https://noema.kevinpei.com';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const results=[];
try {
for(const width of [1440,390]) {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});
 const page=await context.newPage();page.setDefaultTimeout(15000);
 const cases=[
  {name:'Task',path:'/tasks/task%3Af08092849e214992666a670b7626aa46',wait:()=>page.getByRole('heading',{name:'Migration audit — direct execution',exact:true}).waitFor()},
  {name:'Project',path:'/tasks/projects/project%3Aeb7e02135a057b326b0863dbccf19894',wait:()=>page.getByRole('heading',{name:/Migration audit/}).first().waitFor()},
  {name:'Memory',path:'/memory',wait:()=>page.locator('[data-slot=memory-surface]').waitFor()},
  {name:'Settings',path:'/settings/tools/web',wait:()=>page.getByRole('heading',{name:'Web',exact:true}).waitFor()},
  {name:'Unknown path',path:'/audit-unknown-direct-link',wait:()=>page.getByRole('textbox',{name:'Message',exact:true}).waitFor()}
 ];
 for(const item of cases) {
  await page.goto(origin+item.path);let passed=true;await item.wait().catch(()=>{passed=false});
  const result={width,name:item.name,requestedPath:item.path,finalPath:new URL(page.url()).pathname,passed};
  if(!passed)result.observedText=(await page.locator('main').innerText()).slice(0,1500);
  if(item.name==='Memory')result.articleLinks=await page.locator('a[href^="/memory/"]').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('href')));
  results.push(result);
 }
 await context.close();
}
console.log(JSON.stringify({results}));await writeFile('/var/tmp/noema-suite-run-20260905/direct-links-results.json',JSON.stringify({results},null,2)+'\n');
}finally{await browser.close()}
