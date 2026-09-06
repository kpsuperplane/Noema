import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const origin='https://noema.kevinpei.com', evidence={instance:origin,scope:'Keyboard activation with existing virtual passkey. Physical authenticator prompts are not tested.',clients:[]};
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try {
 for(const width of [1440,390]) {
  const context=await browser.newContext({serviceWorkers:'block',viewport:{width,height:1000}}),page=await context.newPage();
  const record={width};evidence.clients.push(record);
  try {
   const cdp=await context.newCDPSession(page);await cdp.send('WebAuthn.enable');
   const {authenticatorId}=await cdp.send('WebAuthn.addVirtualAuthenticator',{options:{protocol:'ctap2',transport:'internal',hasResidentKey:true,hasUserVerification:true,isUserVerified:true,automaticPresenceSimulation:true}});
   const saved=JSON.parse(await readFile('/var/tmp/noema-audit-credentials/virtual-passkey.json','utf8'));
   for(const credential of saved.credentials)await cdp.send('WebAuthn.addCredential',{authenticatorId,credential});
   await page.goto(origin);
   const button=page.getByRole('button',{name:'Use passkey',exact:true});await button.waitFor();
   let reached=false;
   for(let step=0;step<30;step++) {
    if(await button.evaluate(element=>element===document.activeElement)){reached=true;record.tabSteps=step;break;}
    await page.keyboard.press('Tab');
   }
   assert.ok(reached);
   record.focus=await button.evaluate(element=>({focusVisible:element.matches(':focus-visible'),outline:getComputedStyle(element).outline}));
   await page.screenshot({path:`/var/tmp/noema-suite-run-20260905/keyboard-login-${width}.png`,animations:'disabled'});
   const response=page.waitForResponse(response=>response.url().endsWith('/auth/passkey/login/finish'));
   await page.keyboard.press('Enter');
   record.loginStatus=(await response).status();
   assert.equal(record.loginStatus,204);
   await page.getByRole('textbox',{name:'Message',exact:true}).waitFor();
   record.chatComposerVisible=true;
   await page.reload();
   await page.getByRole('textbox',{name:'Message',exact:true}).waitFor();
   record.authenticatedReload=true;
  } finally {
   const logout=await context.request.post(origin+'/auth/logout',{headers:{Origin:origin}});
   record.cleanupStatus=logout.status();assert.equal(record.cleanupStatus,204);
   await context.close();
  }
 }
} catch(error){evidence.error=String(error);throw error;}
finally{await writeFile('docs/validation/evidence/2026-09-06-keyboard-login-results.json',JSON.stringify(evidence,null,2)+'\n');await browser.close();}
