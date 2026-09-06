import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {writeFile} from 'node:fs/promises';
const root='/var/tmp/noema-suite-run-20260905';const records=[];
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{for(const width of [1440,390]){
const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json',viewport:{width,height:1000}});const page=await context.newPage();
await page.goto('https://noema.kevinpei.com/settings/agents');await page.getByRole('button',{name:'Add ACP agent',exact:true}).click();
const name='Migration audit ACP café 日本語 '+width;
await page.getByRole('textbox',{name:/^Name/}).fill(name);await page.getByRole('textbox',{name:/^Executable/}).fill('/var/tmp/noema-audit-missing-executable');await page.getByRole('button',{name:'Add agent',exact:true}).click();
await page.getByRole('button',{name:'Actions for '+name,exact:true}).click();await page.getByRole('menuitem',{name:'Test',exact:true}).click();
await page.getByText(/could not|failed|unavailable/i).last().waitFor();
await page.screenshot({path:root+'/acp-settings-'+width+'.png',fullPage:true});
records.push({width,name,text:(await page.locator('body').innerText()).split('ACP task executors')[1]});
await page.getByRole('button',{name:'Actions for '+name,exact:true}).click();await page.getByRole('menuitem',{name:'Edit',exact:true}).click();
await page.getByRole('textbox',{name:/^Name/}).fill(name+' edited');await page.getByRole('button',{name:'Save',exact:true}).click();
await page.getByRole('button',{name:'Actions for '+name+' edited',exact:true}).click();await page.getByRole('menuitem',{name:'Delete',exact:true}).click();await page.getByRole('button',{name:'Delete executor',exact:true}).click();
await page.getByRole('button',{name:'Actions for '+name+' edited',exact:true}).waitFor({state:'detached'});records.at(-1).editedAndDeleted=true;await context.close();
}}finally{await writeFile(root+'/acp-settings-results.json',JSON.stringify(records,null,2)+'\n');await browser.close();}
