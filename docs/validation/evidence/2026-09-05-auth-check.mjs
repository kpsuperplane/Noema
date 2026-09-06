import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { mkdir, chown, readFile, writeFile, chmod } from 'node:fs/promises';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const root = '/var/tmp/noema-suite-run-20260905';
const home = `${root}/Auth home 日本語 ${Date.now()}`;
const origin = 'http://localhost:43737';
await mkdir(home, { recursive: true, mode: 0o700 });
await chown(home, 65534, 65534);
let child, browser;
const evidence = [];
function pass(id, detail) { evidence.push({ id, result: 'pass', detail }); console.log(`PASS ${id}: ${detail}`); }
async function start() {
  child = spawn(`${root}/noema`, ['--listen', '127.0.0.1:43737'], {
    uid: 65534, gid: 65534, cwd: root,
    env: { PATH: process.env.PATH, NOEMA_HOME: home }, stdio: ['ignore','pipe','pipe']
  });
  await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', code => reject(new Error(`server exited ${code}`)));
    child.stdout.on('data', data => { if (data.toString().includes('Noema listening')) resolve(); });
    child.stderr.on('data', data => process.stderr.write(data));
  });
}
async function stop() { if(child && child.exitCode === null) { const done = once(child, 'exit'); child.kill('SIGTERM'); await done; } }
async function client() {
  const context = await browser.newContext({ serviceWorkers: 'block' });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', { options: {
    protocol: 'ctap2', transport: 'internal', hasResidentKey: true,
    hasUserVerification: true, isUserVerified: true, automaticPresenceSimulation: true
  }});
  await page.goto(origin);
  return { context, page, cdp, authenticatorId };
}
async function call(page, path, body, headers = {}) {
  return page.evaluate(async ({path, body, headers}) => {
    const r = await fetch(path, {method: body === undefined ? 'GET' : 'POST', headers: {'content-type':'application/json', ...headers}, body: body === undefined ? undefined : JSON.stringify(body)});
    const text = await r.text(); let data; try { data=JSON.parse(text); } catch { data=text; }
    return {status:r.status, data};
  }, {path, body, headers});
}
async function status(page, value) {
  await page.waitForFunction(async (value) => (await (await fetch('/auth/status')).json()).state === value, value);
}
async function query(page) { return call(page, '/graphql', {query:'{ __typename }'}); }
try {
  await start();
  browser = await chromium.launch({headless:true, args:['--no-sandbox']});
  const a = await client();
  await a.page.getByRole('button', {name:'Create passkey', exact:true}).waitFor();
  assert.equal((await query(a.page)).status,403);
  const beforeArtifact = await call(a.page,'/artifacts/versions/missing/download');
  assert.ok([401,403].includes(beforeArtifact.status));
  pass('HOME-01', 'Fresh packaged Linux release opens passkey setup without existing credentials.');
  await a.page.getByRole('button',{name:'Create passkey',exact:true}).click();
  await status(a.page,'authenticated');
  assert.equal((await query(a.page)).data.data.__typename,'QueryRoot');
  await a.page.screenshot({path:`${root}/auth-after-claim.png`});
  pass('AUTH-01', 'Browser virtual passkey claims the release and admits GraphQL.');
  const keys = (await call(a.page,'/auth/passkeys')).data;
  assert.equal(keys.length,1);
  const removal=await call(a.page,'/auth/passkey/remove',{credentialId:keys[0].credentialId});
  assert.equal(removal.status,409);
  pass('AUTH-04-final-key', 'The final passkey cannot be removed.');
  const stranger=await client();
  await status(stranger.page,'login_required');
  assert.equal((await query(stranger.page)).status,401);
  assert.equal((await call(stranger.page,'/artifacts/versions/missing/download')).status,401);
  const wsDenied=await stranger.page.evaluate(() => new Promise(resolve => {
    const ws=new WebSocket(`${location.origin.replace('http','ws')}/graphql/ws`,'graphql-transport-ws');
    ws.onopen=()=>resolve(false); ws.onerror=()=>resolve(true);
  }));
  assert.equal(wsDenied,true);
  pass('AUTH-08', 'Unclaimed and signed-out requests cannot read GraphQL or private artifacts; signed-out WebSocket is denied.');
  const hostile = await a.context.request.post(origin+'/graphql',{headers:{origin:'http://attacker.invalid'},data:{query:'{ __typename }'}});
  assert.equal(hostile.status(),403);
  const wrongHost=await a.context.request.get(origin+'/auth/status',{headers:{host:'attacker.invalid'}});
  assert.equal(wrongHost.status(),400);
  pass('AUTH-09', 'Authenticated requests with foreign Origin or Host are rejected.');
  await stop(); await start();
  await a.page.reload(); await status(a.page,'authenticated');
  assert.equal((await query(a.page)).data.data.__typename,'QueryRoot');
  pass('AUTH-03-server-restart', 'Stored browser session survives server restart.');
  const tab=await a.context.newPage(); await tab.goto(origin);
  assert.equal((await call(a.page,'/auth/logout',{})).status,204);
  assert.equal((await query(tab)).status,401);
  assert.equal((await call(tab,'/graphql',{query:'mutation { __typename }'})).status,401);
  pass('AUTH-07-server', 'Logout revokes protected reads and writes in another tab.');
  await a.page.reload(); await a.page.getByRole('button',{name:'Use passkey',exact:true}).click();
  await status(a.page,'authenticated');
  pass('AUTH-06-fresh', 'A fresh passkey ceremony works after logout.');
  await stranger.page.reload();
  await stranger.page.getByRole('button',{name:'Recover access',exact:true}).click();
  const codeField = stranger.page.getByLabel('Recovery code');
  // Read the disposable recovery credential only into the protected flow.
  const config=await readFile(`${home}/config.yaml`,'utf8');
  const code=config.match(/recovery_code:\s*([^\s]+)/)[1].replace(/^['"]|['"]$/g,'');
  await codeField.fill(code);
  await stranger.page.getByRole('button',{name:'Continue',exact:true}).click();
  await stranger.page.getByText('Your access is restored',{exact:true}).waitFor();
  assert.equal((await call(stranger.page,'/auth/recovery',{code})).status,401);
  pass('AUTH-05', 'Recovery enrolls a new virtual passkey; the consumed recovery code is rejected.');
} finally {
  await writeFile(`${root}/auth-results.json`,JSON.stringify(evidence,null,2)+'\n');
  await browser?.close(); await stop();
}
