import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile} from 'node:fs/promises';
const root='/var/tmp/noema-suite-run-20260905',source=JSON.parse(await readFile(root+'/task-edit-source.json','utf8'));
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {for(const width of [1280,390]){
 const context=await browser.newContext({viewport:{width,height:900},serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();await page.goto(source.route);await page.getByRole('button',{name:'Edit description',exact:true}).click();await page.getByRole('button',{name:'Edit source',exact:true}).click();await page.getByRole('textbox',{name:'Task description Markdown source',exact:true}).waitFor();await page.waitForTimeout(500);await page.screenshot({path:root+'/task-edit-'+width+'.png',fullPage:true});await context.close();
}}finally{await browser.close();}
