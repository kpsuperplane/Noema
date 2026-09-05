import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const root='/var/tmp/noema-suite-run-20260905';const evidence=JSON.parse(await readFile(root+'/public-run-results.json','utf8'));
const final=evidence.observed.at(-1);assert.equal(evidence.timeout,false);assert.equal(final.stage.key,'done');
assert.ok(final.resultDocument.includes('42'));assert.ok(final.resultDocument.includes('600'));assert.ok(final.resultDocument.includes('café 日本語 🧭'));assert.ok(final.reviewDocument.length>0);
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 const context=await browser.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'});const page=await context.newPage();
 await page.goto(evidence.route);await page.getByText('café 日本語 🧭',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Task',exact:true}).click();
 await page.getByRole('heading',{name:'Calculation audit',exact:true}).waitFor();
 await page.getByRole('button',{name:'Review',exact:true}).click();
 await page.getByText('RESULT.md satisfies all requirements:',{exact:false}).waitFor();
 await page.getByRole('button',{name:'Transcript',exact:true}).click();
 const detail=page.getByRole('region',{name:'Task detail',exact:true});
 const transcript=page.getByRole('region',{name:'Transcript',exact:true});await transcript.waitFor();
 console.log('Transcript:',(await transcript.innerText()).slice(-2500));
 evidence.documentsVerified='Request, result, and review render separately. The Transcript view opens.';
 evidence.resultVerified=true;evidence.browserReopen='The completed Task displays the Unicode result on reopen.';
 await writeFile(root+'/public-run-results.json',JSON.stringify(evidence,null,2)+'\n');
}finally{await browser.close();}
