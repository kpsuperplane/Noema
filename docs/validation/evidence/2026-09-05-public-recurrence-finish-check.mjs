import http from 'node:http';
import {writeFile} from 'node:fs/promises';
import assert from 'node:assert/strict';
const evidence=[],root='/var/tmp/noema-suite-run-20260905';
const call=(query,variables={})=>new Promise((resolve,reject)=>{const r=http.request({socketPath:'/tmp/noema-codex/graphql.sock',path:'/graphql',method:'POST',headers:{'content-type':'application/json'}},res=>{let text='';res.on('data',d=>text+=d);res.on('end',()=>resolve(JSON.parse(text)));});r.on('error',reject);r.end(JSON.stringify({query,variables}));});
async function gql(q,v){const r=await call(q,v);assert.equal(r.errors,undefined,JSON.stringify(r.errors));return r.data;}
const pass=(id,detail,extra={})=>{evidence.push({id,result:'pass',detail,...extra});console.log('PASS',id,detail);};
const fields='taskId title revision generation taskDocument stage{key} schedule{scheduledFor timeZone recurrenceId} runs{runId kind status}';
const command=async(name,type,input)=>(await gql(`mutation($input:${type}!){${name}(input:$input){task{${fields}}}}`,{input}))[name].task;
const identity=task=>({taskId:task.taskId,expectedRevision:task.revision,expectedGeneration:task.generation,clientMutationId:crypto.randomUUID()});
const readTask=async id=>(await gql(`query($id:String!){task(taskId:$id){${fields}}}`,{id})).task;
const capture=async(title,schedule)=>command('captureTask','CaptureTaskInput',{workspaceId:'workspace:personal',title,taskDocument:'# Scheduling audit\n\nPut the exact text scheduling audit complete in RESULT.md. Do not use external services.',schedule,clientMutationId:crypto.randomUUID()});
let dueTask;
try {
 const start=new Date(Date.now()+86400000);start.setUTCSeconds(0,0);const stamp=start.toISOString();
 const existing=(await gql('{tasks(input:{workspaceId:"workspace:personal",scope:ALL},first:50){edges{node{taskId title}}}}')).tasks.edges.map(x=>x.node).find(x=>x.title==='Migration audit — recurrence commands');
 assert.ok(existing);const recurring=await readTask(existing.taskId);
 const rid=recurring.schedule.recurrenceId;assert.ok(rid);
 const rf='recurrenceId title taskDocument taskDocumentDigest revision lifecycle nextRunAt occurrences{taskId trigger resolution scheduledFor recurrenceRevision}';
 const read=async()=>(await gql(`query($id:String!){taskRecurrence(recurrenceId:$id){${rf}}}`,{id:rid})).taskRecurrence;
 let recurrence=await read();const updated=recurrence.taskDocument;
 await command('cancelTask','CancelTaskInput',{...identity(recurring),reason:'Audit: free the recurrence for a manual occurrence.'});
 const edit='mutation($input:UpdateTaskRecurrenceInput!){updateTaskRecurrence(input:$input){__typename}}';
 const act=async name=>{await gql(`mutation($input:TaskRecurrenceCommandInput!){${name}(input:$input){__typename}}`,{input:{recurrenceId:rid,expectedRevision:recurrence.revision,clientMutationId:crypto.randomUUID()}});recurrence=await read();};
 const cadence=recurrence.nextRunAt;await act('runTaskRecurrenceNow');assert.equal(recurrence.nextRunAt,cadence);
 const manual=recurrence.occurrences.filter(x=>x.trigger==='MANUAL');assert.equal(manual.length,1);assert.ok(manual[0].taskId);
 assert.equal((await readTask(manual[0].taskId)).taskDocument,updated);assert.equal((await readTask(recurring.taskId)).taskDocument,recurring.taskDocument);
 pass('TIME-06-manual','A new manual occurrence copies the updated template. The original Task document remains unchanged.',{recurrenceId:rid,taskId:manual[0].taskId});
 pass('TIME-09-server','Run recurrence now creates one manual occurrence and preserves the next normal slot.',{recurrenceId:rid,normalSlot:cadence});
 await act('endTaskRecurrence');assert.equal(recurrence.lifecycle,'ENDED');assert.equal(recurrence.taskDocument,updated);
 const ended=await call(edit,{input:{recurrenceId:rid,expectedRevision:recurrence.revision,title:'Forbidden ended edit',clientMutationId:crypto.randomUUID()}});assert.ok(ended.errors?.length);
 pass('TIME-08-end','Ended template remains readable and rejects editing.',{recurrenceId:rid});
} finally {await writeFile(root+'/public-recurrence-finish-results.json',JSON.stringify({instance:'https://noema.kevinpei.com',transport:'Development socket',evidence,dueTask},null,2)+'\n');}
