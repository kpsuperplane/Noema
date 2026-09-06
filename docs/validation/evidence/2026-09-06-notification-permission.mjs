import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
const results=[];
try {
for(const width of [1440,390]) {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});
 await context.addInitScript(()=>{
  const original=window.matchMedia.bind(window);
  window.matchMedia=query=>{const result=original(query);if(query.includes('display-mode: standalone'))Object.defineProperty(result,'matches',{value:true});return result};
  window.auditPermissionCalls=0;
  Object.defineProperty(Notification,'permission',{configurable:true,get:()=>window.auditPermissionCalls?'denied':'default'});
  Notification.requestPermission=async()=>{await new Promise(resolve=>setTimeout(resolve,100));window.auditPermissionCalls++;return 'denied'};
 });
 const page=await context.newPage();page.setDefaultTimeout(15000);
 await page.goto('https://noema.kevinpei.com/settings/system/notifications');
 const control=page.getByRole('switch',{name:'Device notifications',exact:true});
 await control.waitFor();
 if(await page.evaluate(()=>window.auditPermissionCalls)!==0)throw Error('Permission requested before user action');
 await control.click();
 const deniedText=page.getByText("Notifications are blocked in this device's system settings.",{exact:true});
 await deniedText.first().waitFor().catch(async error=>{console.log(JSON.stringify({width,calls:await page.evaluate(()=>window.auditPermissionCalls),text:await page.locator("main").innerText()}));});
 if(await page.evaluate(()=>window.auditPermissionCalls)!==1)throw Error('Permission request count differs');
 const screenshot=`/var/tmp/noema-suite-run-20260905/notification-permission-${width}.png`;
 await page.screenshot({path:screenshot,fullPage:true,animations:'disabled'});
 results.push({width,permissionCalls:1,deniedStateVisible:await deniedText.evaluateAll(nodes=>nodes.some(node=>{const style=getComputedStyle(node);return style.display!=='none'&&style.visibility!=='hidden'&&node.getBoundingClientRect().width>0&&node.getBoundingClientRect().height>0;})),permission:await page.evaluate(()=>Notification.permission),screenshot});
 await context.close();
}
await writeFile('/var/tmp/noema-suite-run-20260905/notification-permission-results.json',JSON.stringify({results},null,2)+'\n');console.log(JSON.stringify({results}));
}finally{await browser.close()}
