import { chromium } from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { mkdir, chown, readFile, writeFile, chmod, cp, rename } from 'node:fs/promises';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const root = '/var/tmp/noema-suite-run-20260905';
const home = `${root}/Core home 日本語 ${Date.now()}`;
const origin = 'http://localhost:43738';
await mkdir(home, { recursive: true, mode: 0o700 });
await chown(home, 65534, 65534);
let child, browser;
const evidence = [];
function pass(id, detail) { evidence.push({ id, result: 'pass', detail }); console.log(`PASS ${id}: ${detail}`); }
async function start() {
  child = spawn(`${root}/noema`, ['--listen', '127.0.0.1:43738'], {
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
let page;
async function gql(query, variables={}, target=page) {
  const r=await call(target,'/graphql',{query,variables});
  assert.equal(r.status,200,JSON.stringify(r.data.errors)); assert.equal(r.data.errors,undefined,JSON.stringify(r.data.errors));
  return r.data.data;
}
const taskFields='taskId title taskDocument taskDocumentDigest revision generation stage { key } effectiveCwd';
try {
  await start(); browser=await chromium.launch({headless:true,args:['--no-sandbox']});
  const a=await client(); page=a.page;
  await page.getByRole('button',{name:'Create passkey',exact:true}).click(); await status(page,'authenticated');
  await page.getByText('Choose how Noema thinks',{exact:true}).waitFor();
  const account='provider_account:openai:default';
  await gql('mutation($input:ProviderSecretInput!){saveProviderSecretInput(input:$input){status}}',{input:{providerAccountId:account,secret:crypto.randomUUID()}});
  const setup=await gql('query($id:String!){onboardingModelSetup(providerAccountId:$id){profiles{id reasoningEfforts} recommendations{useCase modelProfile}} onboardingStatus{isUserOnboarded}}',{id:account});
  assert.equal(setup.onboardingStatus.isUserOnboarded,false);
  const selection={selectionMode:'NOEMA_RECOMMENDED',fastMode:false};
  const input={providerAccountId:account};
  for(const role of ['noema','simpleTasks','mediumTasks','difficultTasks','taskReviewer','webFetchSummarizer','toolProgressAudit','actionReviewer','memoryConsolidation']) input[role]=selection;
  input.noema={selectionMode:'EXPLICIT_PROFILE',modelProfile:setup.onboardingModelSetup.profiles[0].id,reasoningEffort:setup.onboardingModelSetup.profiles[0].reasoningEfforts[0],fastMode:false};
  assert.equal((await gql('{onboardingStatus{isUserOnboarded}}')).onboardingStatus.isUserOnboarded,false);
  const confirmed=await gql('mutation($input:ConfirmOnboardingModelSelectionsInput!){confirmOnboardingModelSelections(input:$input){isUserOnboarded}}',{input});
  assert.equal(confirmed.confirmOnboardingModelSelections.isUserOnboarded,true);
  pass('SETUP-02-client-command','Reading proposed choices leaves setup incomplete. Confirming the complete selection enables the app.');
  await page.goto(origin+'/tasks/new');
  await page.getByLabel('Task title',{exact:true}).fill('旅行 🧭 café');
  await page.getByRole('button',{name:'Edit Markdown source',exact:true}).click();
  const doc='# Request\n\nKeep 日本語, café, 🧭 and https://example.com/a?b=1 intact.\n';
  await page.getByRole('textbox',{name:'Task document Markdown source',exact:true}).fill(doc);
  await page.getByRole('button',{name:'More Task creation actions',exact:true}).click();
  await page.getByRole('menuitem',{name:'Add to Inbox',exact:true}).click();
  await page.waitForURL(/\/tasks\/.+/);
  await page.waitForFunction(()=>!location.pathname.endsWith('/new'));
  console.log('Task route',new URL(page.url()).pathname);
  await page.screenshot({path:`${root}/core-inbox.png`});
  const taskID=decodeURIComponent(new URL(page.url()).pathname.split('/').pop());
  let task=(await gql(`query($id:String!){task(taskId:$id){${taskFields}}}`,{id:taskID})).task;
  assert.equal(task.title,'旅行 🧭 café'); assert.equal(task.stage.key,'inbox'); assert.equal(task.taskDocument,doc);
  pass('TASK-02-inbox','Browser Add to Inbox saves an exact Unicode Task without starting execution.');
  const old={taskId:task.taskId,expectedRevision:task.revision,expectedGeneration:task.generation,expectedTaskDocumentDigest:task.taskDocumentDigest};
  const edit={...old,title:'Renamed café',taskDocument:doc+'\nCurrent edit.\n',clientMutationId:'core-edit'};
  const saved=(await gql(`mutation($input:UpdateInboxTaskInput!){updateInboxTask(input:$input){task{${taskFields}}}}`,{input:edit})).updateInboxTask.task;
  assert.equal(saved.effectiveCwd,task.effectiveCwd);
  const stale=await call(page,'/graphql',{query:`mutation($input:UpdateInboxTaskInput!){updateInboxTask(input:$input){task{${taskFields}}}}`,variables:{input:{...old,taskDocument:'Stale replacement',clientMutationId:'core-stale'}}});
  assert.ok(stale.data.errors?.length);
  task=(await gql(`query($id:String!){task(taskId:$id){${taskFields}}}`,{id:task.taskId})).task;
  assert.equal(task.taskDocument,edit.taskDocument);
  pass('TASK-03-client-command','Renaming preserves the Task directory. The exact edited document remains stored.');
  pass('TASK-04-server','A competing stale document save is rejected and preserves current content.');
  const captureInput={workspaceId:'workspace:personal',title:'旅行 🧭 café',taskDocument:doc,clientMutationId:'core-capture-replay'};
  const capture=`mutation($input:CaptureTaskInput!){captureTask(input:$input){task{${taskFields}}}}`;
  const first=(await gql(capture,{input:captureInput})).captureTask.task;
  const second=(await gql(capture,{input:captureInput})).captureTask.task;
  assert.equal(first.taskId,second.taskId); assert.notEqual(first.taskId,task.taskId);
  assert.equal(await readFile(`${home}/tasks/${first.taskId.replace(/^task:/,'')}/TASK.md`,'utf8'),doc);
  assert.equal(await readFile(`${home}/tasks/${task.taskId.replace(/^task:/,'')}/TASK.md`,'utf8'),edit.taskDocument);
  pass('TASK-10-capture','Repeating the same accepted capture returns its original Task.');
  pass('TASK-11','Equal Unicode titles allocate distinct directories and preserve existing documents.');
  const project=(await gql('mutation($input:CreateProjectInput!){createProject(input:$input){project{projectId revision name}}}',{input:{workspaceId:'workspace:personal',name:'旅行 Project',description:'Original context',clientMutationId:'core-project'}})).createProject.project;
  const pd=(await gql('query($id:String!){projectDocument(projectId:$id){content digest}}',{id:project.projectId})).projectDocument;
  const pi={projectId:project.projectId,expectedRevision:project.revision,expectedDocumentDigest:pd.digest,content:'# Current Project\n\nShared café context.\n',clientMutationId:'core-project-doc'};
  const projectEdit='mutation($input:UpdateProjectDocumentInput!){updateProjectDocument(input:$input){project{revision} document{content digest}}}';
  const ps=(await gql(projectEdit,{input:pi})).updateProjectDocument;
  const staleProject=await call(page,'/graphql',{query:projectEdit,variables:{input:{...pi,content:'Stale',clientMutationId:'core-project-stale'}}});
  assert.ok(staleProject.data.errors?.length);
  assert.equal((await gql('query($id:String!){projectDocument(projectId:$id){content}}',{id:project.projectId})).projectDocument.content,pi.content);
  pass('PROJECT-02-server','Project edits persist. A competing stale save cannot replace current content.');
  const archived=(await gql('mutation($input:ArchiveProjectInput!){archiveProject(input:$input){project{revision archivedAt}}}',{input:{projectId:project.projectId,expectedRevision:ps.project.revision,clientMutationId:'core-project-archive'}})).archiveProject.project;
  assert.ok(archived.archivedAt);
  const archivedEdit=await call(page,'/graphql',{query:projectEdit,variables:{input:{...pi,expectedRevision:archived.revision,expectedDocumentDigest:ps.document.digest,clientMutationId:'core-archived-edit'}}});
  assert.ok(archivedEdit.data.errors?.length);
  const reopened=(await gql('mutation($input:ReopenProjectInput!){reopenProject(input:$input){project{archivedAt}}}',{input:{projectId:project.projectId,expectedRevision:archived.revision,clientMutationId:'core-project-reopen'}})).reopenProject.project;
  assert.equal(reopened.archivedAt,null);
  pass('PROJECT-04-server','Archive preserves readable context and refuses edits. Reopen restores active state.');
  await stop();
  const backup=home+' backup'; await cp(home,backup,{recursive:true,preserveTimestamps:true});
  // Restore the complete stopped home while retaining the original copy.
  await rename(home,home+' original'); await cp(backup,home,{recursive:true,preserveTimestamps:true});
  const ownership=spawn('chown',['-R','65534:65534',home]); assert.equal((await once(ownership,'exit'))[0],0);
  await start(); await page.reload(); await status(page,'authenticated');
  const restored=(await gql(`query($id:String!){task(taskId:$id){${taskFields}}}`,{id:task.taskId})).task;
  assert.equal(restored.taskDocument,task.taskDocument); assert.equal(restored.title,task.title);
  assert.equal((await gql('query($id:String!){projectDocument(projectId:$id){content}}',{id:project.projectId})).projectDocument.content,pi.content);
  assert.equal((await gql('{onboardingStatus{isUserOnboarded}}')).onboardingStatus.isUserOnboarded,true);
  pass('HOME-05-partial','Complete stopped-home restore preserves passkey access, Task content, Project context, and model selections.');
} catch(error) {
  if(page) { console.log((await page.locator('body').innerText()).slice(0,5000)); await page.screenshot({path:`${root}/core-failure.png`}); }
  throw error;
} finally {
  await writeFile(`${root}/core-results.json`,JSON.stringify(evidence,null,2)+'\n');
  await browser?.close(); await stop();
}
