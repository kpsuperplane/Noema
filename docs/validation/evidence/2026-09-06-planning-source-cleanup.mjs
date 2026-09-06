import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const file='/root/noema/docs/validation/evidence/2026-09-06-planning-journeys-results.json',e=JSON.parse(await readFile(file,'utf8'));
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();await p.goto(e.instance);
const query=async(query,variables)=>p.evaluate(async({query,variables})=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query,variables})})).json(),{query,variables});
e.tasksAfter=[];for(const task of e.tasks){const r=await query('query($id:String!){task(taskId:$id){taskId stage{key} revision}}',{id:task.taskId});assert.equal(r.errors,undefined);assert.equal(r.data.task.stage.key,'inbox');e.tasksAfter.push(r.data.task);}
const r=await query('mutation($id:String!){deleteMcpServer(mcpServerId:$id)}',{id:e.server.mcpServerId});assert.equal(r.errors,undefined);assert.equal(r.data.deleteMcpServer,true);
const after=await query('{mcpServers{mcpServerId}}',{});assert.equal(after.errors,undefined);assert.ok(!after.data.mcpServers.some(s=>s.mcpServerId===e.server.mcpServerId));e.cleanup={temporaryIntegrationRemoved:true};await writeFile(file,JSON.stringify(e,null,2)+'\n');
}finally{await b.close();}
