import {chromium} from '/tmp/bunx-0-playwright@latest/node_modules/playwright/index.mjs';
import {readFile,writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const file='/root/noema/docs/validation/evidence/2026-09-06-project-research-results.json',e=JSON.parse(await readFile(file,'utf8'));
const b=await chromium.launch({headless:true,args:['--no-sandbox','--host-resolver-rules=MAP noema.kevinpei.com 127.0.0.1']});
try{
 const c=await b.newContext({serviceWorkers:'block',storageState:'/var/tmp/noema-audit-credentials/browser-session.json'}),p=await c.newPage();
 await p.goto(e.instance+'/tasks/'+encodeURIComponent(e.taskId)+'?terminal=all');await p.getByRole('button',{name:'Result',exact:true}).click();
 await p.getByText('AUDIT_RESEARCH_CURRENT',{exact:true}).first().waitFor();
 const version=e.final.resultDocument.match(/artifact_version:([a-f0-9]+)/)[1];
 const r=await c.request.get(e.instance+'/artifacts/versions/'+version+'/download');assert.equal(r.status(),200);
 assert.equal(await r.text(),e.report);
 e.artifactVerification={version,status:r.status(),exactReportBytes:true,currentLabelVisible:true};
 const run=e.final.runs.filter(run=>run.kind==='EXECUTOR').at(-1).runId;
 const data=await p.evaluate(async run=>await(await fetch('/graphql',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({query:'query($run:String!){taskRunItems(runId:$run,first:100){edges{node{kind payload}}pageInfo{hasNextPage}}}',variables:{run}})})).json(),run);
 assert.equal(data.errors,undefined);assert.equal(data.data.taskRunItems.pageInfo.hasNextPage,false);
 e.executionTools=data.data.taskRunItems.edges.map(edge=>edge.node).filter(n=>n.kind==='TOOL_RESULT').map(n=>({name:n.payload.name,success:n.payload.success}));
 e.searchEvidence=data.data.taskRunItems.edges.map(edge=>edge.node).filter(n=>n.kind==='ASSISTANT_OUTPUT'&&n.payload?.searches?.length).map(n=>n.payload.searches);
 await writeFile(file,JSON.stringify(e,null,2)+'\n');
}finally{await b.close();}
