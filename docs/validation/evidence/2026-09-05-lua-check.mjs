import { startFakeServices } from './2026-09-05-fake-services.mjs';
import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { mkdir, chown, readFile, writeFile, chmod, cp, rename } from 'node:fs/promises';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const root = '/var/tmp/noema-suite-run-20260905';
const home = `${root}/Document home 日本語 ${Date.now()}`;
const origin = 'http://localhost:43739';
await mkdir(home, { recursive: true, mode: 0o700 });
await chown(home, 65534, 65534);
let child, browser;
const evidence = [];
let fake; const outputs = new Map();
function pass(id, detail) { evidence.push({ id, result: 'pass', detail }); console.log(`PASS ${id}: ${detail}`); }
async function start() {
  child = spawn(`${root}/noema-oauth-response`, ['--listen', '127.0.0.1:43739'], {
    uid: 65534, gid: 65534, cwd: root,
    env: { PATH: process.env.PATH, NOEMA_HOME: home, SSL_CERT_FILE: root+'/fake-ca.pem', NOEMA_OPENAI__API_KEY: crypto.randomUUID() }, stdio: ['ignore','pipe','pipe']
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
let page;
async function gql(query, variables={}, target=page) {
  const r=await call(target,'/graphql',{query,variables});
  assert.equal(r.status,200,JSON.stringify(r.data.errors)); assert.equal(r.data.errors,undefined,JSON.stringify(r.data.errors));
  return r.data.data;
}
try {
  fake=await startFakeServices(body => { for(const item of body.input || []) if(item.type === 'function_call_output') outputs.set(item.call_id, JSON.parse(item.output)); }); await start();
  browser=await chromium.launch({headless:true,args:['--no-sandbox',`--ignore-certificate-errors-spki-list=${fake.pin}`]});
  const a=await client();page=a.page;
  await page.getByRole('button',{name:'Create passkey',exact:true}).click();await status(page,'authenticated');
  const conversation=(await gql('mutation($cwd:String){ensurePrimaryConversation(cwd:$cwd){conversationId}}', {cwd:root+'/Document fixtures 日本語'})).ensurePrimaryConversation.conversationId;
  const send=async(input)=>gql('mutation($input:SendConversationTurnInput!){sendConversationTurn(input:$input){__typename}}',{input:{conversationId:conversation,input,clientMessageId:crypto.randomUUID(),clientTimeZone:'UTC'}});
  const run=async(tool,args)=>{
    const prior=new Set(outputs.keys()); const marker='Completed check '+crypto.randomUUID();
    fake.replies.push({tool,arguments:args},{text:marker});
    await send('Run the supplied controlled '+tool+' check.');
    await page.getByText(marker,{exact:false}).waitFor();
    const result=[...outputs].filter(([id])=>!prior.has(id)).at(-1)?.[1];
    assert.ok(result,'Tool output must reach the provider'); return result;
  };
  const calc=await run('run_lua',{source:'return {remaining=input.budget-input.cost, percent=input.cost/input.budget*100, values=input.values, label=input.label}',input:{budget:2400,cost:600,values:[null,1,{x:true}],label:'café 日本語 🧭'}});
  assert.equal(calc.success,true);assert.deepEqual(calc.payload.value,{remaining:1800,percent:25,values:[null,1,{x:true}],label:'café 日本語 🧭'});
  pass('CALC-01','Budget, percentage, structured JSON, nulls, and Unicode match independent expected values.');
  for(const source of ['while true do end','return io.open("/etc/hostname"):read("*a")','return os.execute("true")','return os.getenv("PATH")','return require("socket")']) {
    const started=Date.now();const result=await run('run_lua',{source});assert.equal(result.success,false);assert.ok(Date.now()-started<15000);
  }
  pass('CALC-02','Unbounded execution and file, process, environment, and network access fail within 15 seconds each.');
} finally {
  await writeFile(root+'/lua-results.json',JSON.stringify(evidence,null,2)+'\n');
  await writeFile(root+'/lua-outputs.json',JSON.stringify([...outputs.values()],null,2)+'\n');
  await browser?.close();await stop();await fake?.close();
}
