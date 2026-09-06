import http from 'node:http';
export async function startPlanningSources() {
 const receipts=[];
 const messages=[
  {id:'message:audit-demo',account:'audit-account',time:'2026-09-07T08:00:00-07:00',subject:'Demo preparation',body:'Prepare the staging demo before Monday 15:00. The release date is not approved.'},
  {id:'message:audit-signoff',account:'audit-account',time:'2026-09-07T08:30:00-07:00',subject:'Acceptance evidence missing',body:'Customer sign-off is missing. Do not report production rollout as completed.'},
  {id:'message:outside-window',account:'audit-account',time:'2026-09-06T08:00:00-07:00',subject:'Prior day record',body:'Outside the requested day.'}
 ];
 const events=[
  {id:'event:standup-1',seriesId:'series:standup',start:'2026-09-07T09:00:00-07:00',end:'2026-09-07T09:30:00-07:00',title:'Weekly standup',location:'Office A',recurring:true},
  {id:'event:review',start:'2026-09-07T10:00:00-07:00',end:'2026-09-07T11:00:00-07:00',title:'Review',location:'Office A'},
  {id:'event:visit',start:'2026-09-07T10:30:00-07:00',end:'2026-09-07T11:30:00-07:00',title:'Customer visit',location:'Office B',travelFromPreviousLocationMinutes:45},
  {id:'event:leave',start:'2026-09-09T00:00:00-07:00',end:'2026-09-10T00:00:00-07:00',allDay:true,date:'2026-09-09',title:'Leave — no work capacity'},
  {id:'event:standup-2',seriesId:'series:standup',start:'2026-09-14T09:00:00-07:00',end:'2026-09-14T09:30:00-07:00',title:'Weekly standup',location:'Office A',recurring:true}
 ];
 const tools=['audit_messages','audit_calendar'].map(name=>({name,description:name==='audit_messages'?'Read bounded synthetic audit messages. Account audit-account. Requires inclusive from and exclusive to instants.':'Read bounded synthetic audit calendar occurrences. Preserves local times, series identity, all-day dates, and explicit travel constraints. Requires inclusive from and exclusive to instants.',inputSchema:{type:'object',properties:{from:{type:'string'},to:{type:'string'}},required:['from','to'],additionalProperties:false},annotations:{readOnlyHint:true,destructiveHint:false,idempotentHint:true,openWorldHint:false}}));
 const server=http.createServer(async(req,res)=>{
  if(req.method!=='POST'){res.writeHead(405);res.end();return;}
  let request;try{const chunks=[];for await(const chunk of req)chunks.push(chunk);request=JSON.parse(Buffer.concat(chunks).toString());}catch{res.writeHead(400);res.end();return;}
  if(request.id===undefined){res.writeHead(202);res.end();return;}
  let result;
  if(request.method==='initialize')result={protocolVersion:request.params.protocolVersion,capabilities:{tools:{}},serverInfo:{name:'Noema controlled planning sources',version:'1'}};
  else if(request.method==='tools/list')result={tools};
  else if(request.method==='tools/call'){
   const {name,arguments:args}=request.params,from=Date.parse(args.from),to=Date.parse(args.to);
   if(!tools.some(tool=>tool.name===name)||!Number.isFinite(from)||!Number.isFinite(to)||from>=to){result={isError:true,content:[{type:'text',text:'Valid bounded dates are required.'}]};}
   else {const rows=(name==='audit_messages'?messages:events).filter(row=>Date.parse(row.time||row.start)>=from&&Date.parse(row.time||row.start)<to);receipts.push({name,from:args.from,to:args.to,ids:rows.map(row=>row.id)});result={isError:false,content:[{type:'text',text:JSON.stringify({account:'audit-account',timeZone:'America/Los_Angeles',from:args.from,to:args.to,records:rows,complete:true})}]};}
  }else if(request.method==='ping')result={};
  else {res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:request.id,error:{code:-32601,message:'Method not found'}}));return;}
  res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:request.id,result}));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 return {url:`http://127.0.0.1:${server.address().port}/mcp`,receipts,close:()=>new Promise(resolve=>server.close(resolve))};
}
