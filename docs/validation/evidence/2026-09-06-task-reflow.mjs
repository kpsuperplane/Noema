import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com',taskId='task:9496e6cdadbccf4f2f4b0ebe64002590';
const evidence={instance:origin,taskId,scope:'Controlled viewport reduction and doubled rendered text sizes. This is not native browser zoom or a physical on-screen keyboard.',states:[]};
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 for(const [width,height,scale] of [[1440,1000,1],[320,480,1],[320,480,2]]) {
  const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height},reducedMotion:'reduce'}),page=await context.newPage();
  const record={width,height,textScale:scale};evidence.states.push(record);
  await page.goto(origin+'/tasks/'+encodeURIComponent(taskId)+'?terminal=all');
  await page.getByRole('button',{name:'Edit description',exact:true}).click();
  await page.getByRole('button',{name:'Edit source',exact:true}).click();
  const input=page.getByRole('textbox',{name:'Task description Markdown source',exact:true});
  const original=await input.inputValue();
  const draft='# Reflow audit\n\nPreserve café 日本語 🧭.\n\n'+Array.from({length:12},(_,i)=>`Line ${i+1}: Keep this document readable.`).join('\n');
  await input.fill(draft);
  record.beforeFont=await input.evaluate(element=>getComputedStyle(element).fontSize);
  if(scale===2)await page.evaluate(()=>{
   const elements=[...document.querySelectorAll('body *')].filter(element=>element instanceof HTMLElement && (element.matches('input,textarea') || [...element.childNodes].some(node=>node.nodeType===Node.TEXT_NODE && node.textContent.trim())));
   const sizes=elements.map(element=>({element,size:parseFloat(getComputedStyle(element).fontSize),line:parseFloat(getComputedStyle(element).lineHeight)}));
   for(const {element,size,line} of sizes){element.style.fontSize=size*2+'px';if(Number.isFinite(line))element.style.lineHeight=line*2+'px';}
  });
  record.afterFont=await input.evaluate(element=>getComputedStyle(element).fontSize);
  assert.equal(parseFloat(record.afterFont),parseFloat(record.beforeFont)*scale);
  await input.scrollIntoViewIfNeeded();
  record.input=await input.evaluate(element=>{const r=element.getBoundingClientRect();return {width:r.width,height:r.height,left:r.left,right:r.right,top:r.top,bottom:r.bottom,viewportWidth:innerWidth,viewportHeight:innerHeight};});
  record.draftPreserved=await input.inputValue()===draft;
  const save=page.getByRole('button',{name:'Save description',exact:true});await save.scrollIntoViewIfNeeded();
  record.save=await save.evaluate(element=>{const r=element.getBoundingClientRect();const hit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2);return {visible:r.left>=0&&r.right<=innerWidth&&r.top>=0&&r.bottom<=innerHeight,hit:hit===element||element.contains(hit)};});
  await page.screenshot({path:`/var/tmp/noema-suite-run-20260905/task-reflow-${width}-${height}-${scale}.png`,animations:'disabled'});
  await page.getByRole('button',{name:'Cancel Description edit',exact:true}).click();
  await page.getByRole('button',{name:'Edit description',exact:true}).click();await page.getByRole('button',{name:'Edit source',exact:true}).click();
  record.cancelPreservesOriginal=await input.inputValue()===original;assert.ok(record.cancelPreservesOriginal&&record.draftPreserved);
  await context.close();
 }
}catch(error){evidence.error=String(error);throw error;}
finally{await writeFile('docs/validation/evidence/2026-09-06-task-reflow-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
