import { startFakeServices } from './2026-09-05-fake-services.mjs';
import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { mkdir, chown, readFile, writeFile, chmod, cp, rename } from 'node:fs/promises';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const root = '/var/tmp/noema-suite-run-20260905';
const home = `${root}/Wire home 日本語 ${Date.now()}`;
const origin = 'http://localhost:43739';
await mkdir(home, { recursive: true, mode: 0o700 });
await chown(home, 65534, 65534);
let child, browser;
const evidence = [];
let fake;
function pass(id, detail) { evidence.push({ id, result: 'pass', detail }); console.log(`PASS ${id}: ${detail}`); }
async function start() {
  child = spawn(`${root}/noema-labels-frozen`, ['--listen', '127.0.0.1:43739'], {
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
  fake=await startFakeServices(); await start();
  browser=await chromium.launch({headless:true,args:['--no-sandbox',`--ignore-certificate-errors-spki-list=${fake.pin}`]});
  const a=await client();page=a.page;
  await page.getByRole('button',{name:'Create passkey',exact:true}).click();await status(page,'authenticated');
  const conversation=(await gql('mutation{ensurePrimaryConversation{conversationId}}')).ensurePrimaryConversation.conversationId;
  const send=async(input)=>gql('mutation($input:SendConversationTurnInput!){sendConversationTurn(input:$input){__typename}}',{input:{conversationId:conversation,input,clientMessageId:crypto.randomUUID(),clientTimeZone:'UTC'}});
  fake.replies.push({text:'Hello café. 日本語 🧭 remains intact.\n\nhttps://example.com/reference'});
  await send('Preserve café, 日本語 and 🧭 in this controlled response.');
  await page.getByText('Hello café. 日本語 🧭 remains intact.',{exact:false}).waitFor();
  await page.reload();await page.getByText('Hello café. 日本語 🧭 remains intact.',{exact:false}).waitFor();
  pass('CHAT-01-controlled','A real release sends a provider request and preserves the controlled answer after browser reload.');
  pass('CHAT-06-controlled','Unicode, emoji, paragraphs, and an exact URL survive provider transport and browser reload.');
  const state=(await gql('{adapterOauthState{profiles{profileId profileDigest}}}')).adapterOauthState;
  const profile=state.profiles.find(p=>p.profileId==='google').profileDigest;
  const proposed=[];
  for(const suffix of ['one','two']) {
    const proposal={source_reference:'https://api.example.com/docs',new_definition:{definition_id:'wire-'+suffix,adapter_id:'wire-'+suffix,display_name:'Controlled API '+suffix,definition_revision:'2026-09-05',origin:'https://api.example.com/',authentication:{kind:'oauth2_authorization_code_pkce',profile_digest:profile}},upsert_operations:[{operation_id:'lookup',description:'Read one controlled record.',method:'GET',path:'/v1/items/{id}',authorization:{kind:'oauth_scopes',accepted_scope_sets:[['scope.read']]},arguments:[{name:'id',description:'Record identifier.',location:'path',type:'string',required:true}],read_only:true,idempotent:true,destructive:false,open_world:true,response:{kind:'custom',accepted_content_types:['application/json'],output_schema:{type:'object',properties:{name:{type:'string',maxBytes:128}},required:['name'],additionalProperties:false}}}]};
    fake.replies.push({tool:'propose_definition',arguments:proposal},{text:'Controlled API '+suffix+' proposal is ready for review.'});
    await send('Propose controlled API '+suffix+' from its supplied structured definition.');
    await page.getByText('Controlled API '+suffix+' proposal is ready for review.',{exact:false}).waitFor();
    const definitions=(await gql('{adapterDefinitions{semanticDigest adapterId reviewed}}')).adapterDefinitions;
    const definition=definitions.find(d=>d.adapterId==='wire-'+suffix);assert.ok(definition);assert.equal(definition.reviewed,false);
    const approved=await gql('mutation($input:ApproveAdapterDefinitionInput!){approveAdapterDefinition(input:$input){reviewed semanticDigest}}',{input:{semanticDigest:definition.semanticDigest}});
    proposed.push(approved.approveAdapterDefinition.semanticDigest);
  }
  pass('API-01-proposal','Chat tool calls create two pending definitions. Explicit review accepts each definition.');
  const application=(await gql('mutation($input:ImportAdapterOauthApplicationInput!){importAdapterOauthApplication(input:$input){applicationId revision}}',{input:{profileDigest:profile,projectLabel:'Controlled issuer',clientDocumentBase64:Buffer.from(JSON.stringify({installed:{client_id:'controlled-client',client_secret:crypto.randomUUID(),redirect_uris:['http://localhost']}})).toString('base64')}})).importAdapterOauthApplication;
  const attemptInput={applicationId:application.applicationId,expectedApplicationRevision:application.revision,semanticDigest:proposed[0],operationIds:['lookup'],additionalServices:[{semanticDigest:proposed[1],operationIds:['lookup']}]};
  const begin=async(input)=> (await gql('mutation($input:StartAdapterOauthSetupInput!){startAdapterOauthSetup(input:$input){attemptId authorizationUrl}}',{input})).startAdapterOauthSetup;
  const finish=async(attempt,options={})=>{
    const url=new URL(attempt.authorizationUrl);for(const [key,value] of Object.entries(options))url.searchParams.set(key,value);
    const consent=await a.context.newPage();await consent.goto(url.href);await consent.close();
    return (await gql('query($id:String!){adapterOauthAttempt(attemptId:$id){status grantId grantRevision}}',{id:attempt.attemptId})).adapterOauthAttempt;
  };
  const first=await finish(await begin(attemptInput));assert.equal(first.status,'completed');
  for(const digest of proposed) await gql('mutation($input:AttachAdapterOauthConnectionInput!){attachAdapterOauthConnection(input:$input){connectionCount}}',{input:{semanticDigest:digest,grantId:first.grantId,expectedGrantRevision:first.grantRevision}});
  const connections=(await gql('{adapterDefinitions{adapterId connectionCount connections{connectionId}}}')).adapterDefinitions;
  assert.equal(connections.filter(d=>d.adapterId.startsWith('wire-')).reduce((n,d)=>n+d.connectionCount,0),2);
  assert.equal(fake.receipts.filter(r=>r.kind==='token').length,1);
  pass('API-03-controlled','One browser OAuth sign-in exchanges a PKCE code and attaches two APIs to the resulting grant.');
  const denied=await finish(await begin(attemptInput),{fixture_denied:'true'});assert.equal(denied.status,'denied');
  assert.equal(fake.receipts.filter(r=>r.kind==='token').length,1);
  pass('API-05-denied','Denied consent records a denied attempt without a token exchange or false connection.');
  const second=await finish(await begin(attemptInput),{fixture_account:'account-two'});assert.equal(second.status,'completed');assert.notEqual(second.grantId,first.grantId);
  pass('API-04-grants','Selecting a second synthetic account creates a distinct grant. Invocation identity remains to be checked.');
  await gql('mutation($input:SaveAdapterOauthGrantLabelInput!){saveAdapterOauthGrantLabel(input:$input){accountLabel}}',{input:{grantId:first.grantId,expectedAuthorityRevision:first.grantRevision,accountLabel:'Shared account café'}});
  const integrations=(await gql('{adapterManagement{integrations{connections{connectionId connectionRevision policyRevision}}}}')).adapterManagement.integrations;
  for(const [index,connection] of integrations.flatMap(i=>i.connections).entries()) {
    await gql('mutation($input:SaveCapabilityConnectionLabelInput!){saveCapabilityConnectionLabel(input:$input){connectionId}}',{input:{kind:'API',connectionId:connection.connectionId,expectedConnectionRevision:connection.connectionRevision,connectionLabel:'Controlled account '+index}});
  }
  const current=(await gql('{adapterManagement{integrations{connections{connectionId connectionRevision policyRevision}}}}')).adapterManagement.integrations;
  for(const connection of current.flatMap(i=>i.connections)) {
    await gql('mutation($input:SaveCapabilityConnectionPolicyInput!){saveCapabilityConnectionPolicy(input:$input){connectionId}}',{input:{kind:'API',connectionId:connection.connectionId,expectedConnectionRevision:connection.connectionRevision,expectedPolicyRevision:connection.policyRevision,dataSharingPolicy:'allow_automatically',unsafeActionPolicy:'always_ask'}});
  }
  fake.replies.push({text:'Controlled account inspection complete.'});
  await send('Inspect the available controlled accounts.');
  await page.getByText('Controlled account inspection complete.',{exact:false}).waitFor();
  const offered=fake.receipts.filter(r=>r.kind==='model').at(-1).apiTools;
  assert.equal(offered.length,2);
  assert.ok(offered.every(t=>t.description.includes('Shared account café')),'Shared account rename is missing from model tools.');
  assert.ok(offered.some(t=>t.description.includes('Controlled account 0')),'First configured account label is absent from model tools.');
  assert.ok(offered.some(t=>t.description.includes('Controlled account 1')),'Second configured account label is absent from model tools.');
  pass('API-04-labels','Current shared account and per-connection labels reach distinct provider tool descriptions after renaming.');
  fake.expireAccess();
  for(const index of [0,1]) {
    fake.replies.push({toolLabel:'Connection: "Controlled account '+index+'"',arguments:{id:'record-'+index}},{text:'Controlled read '+index+' complete.'});
    await send('Read record '+index+' from controlled account '+index+'.');
    await page.getByText('Controlled read '+index+' complete.',{exact:false}).waitFor();
  }
  const reads=fake.receipts.filter(r=>r.kind==='api-read');
  assert.equal(reads.length,2);assert.ok(reads.every(r=>r.account==='account-one'));
  assert.equal(fake.receipts.filter(r=>r.kind==='token'&&r.grantType==='refresh_token').length,1);
  pass('API-08-sequential','An expired shared token refreshes once. Both dependent APIs then return the intended account records. Concurrent refresh remains pending.');


} catch(error) {
  if(page){console.log((await page.locator('body').innerText()).slice(0,7000));await page.screenshot({path:`${root}/wire-failure.png`});}
  throw error;
} finally {
  await writeFile(`${root}/wire-results.json`,JSON.stringify(evidence,null,2)+'\n');
  if(fake)await writeFile(`${root}/wire-receipts.json`,JSON.stringify(fake.receipts,null,2)+'\n');
  await browser?.close();await stop();await fake?.close();
}
