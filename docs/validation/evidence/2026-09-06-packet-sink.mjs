import http from 'node:http';
import {createHash} from 'node:crypto';
export async function startPacketSink(){
 const receipts=[];
 const tool={name:'audit_upload_packet',description:'Upload one approved synthetic private packet to destination audit-packet-box. Binds the exact Artifact ID, version ID, and Markdown bytes. Returns a verifiable receipt. This operation writes a receipt and must not be repeated.',inputSchema:{type:'object',properties:{artifact_id:{type:'string'},artifact_version_id:{type:'string'},packet_markdown:{type:'string'},destination:{type:'string',enum:['audit-packet-box']}},required:['artifact_id','artifact_version_id','packet_markdown','destination'],additionalProperties:false},annotations:{readOnlyHint:false,destructiveHint:false,idempotentHint:false,openWorldHint:false}};
 const server=http.createServer(async(req,res)=>{
  if(req.method!=='POST'){res.writeHead(405);res.end();return;}
  const chunks=[];for await(const chunk of req)chunks.push(chunk);const request=JSON.parse(Buffer.concat(chunks).toString());
  if(request.id===undefined){res.writeHead(202);res.end();return;}
  let result;
  if(request.method==='initialize')result={protocolVersion:request.params.protocolVersion,capabilities:{tools:{}},serverInfo:{name:'Noema audit packet sink',version:'1'}};
  else if(request.method==='tools/list')result={tools:[tool]};
  else if(request.method==='tools/call'&&request.params.name===tool.name){
   const args=request.params.arguments;
   if(args.destination!=='audit-packet-box'||typeof args.packet_markdown!=='string')result={isError:true,content:[{type:'text',text:'Invalid packet destination or content.'}]};
   else {const receipt={receipt_id:'audit-receipt-'+(receipts.length+1),...args,sha256:createHash('sha256').update(args.packet_markdown).digest('hex'),bytes:Buffer.byteLength(args.packet_markdown)};receipts.push(receipt);const {packet_markdown,...publicReceipt}=receipt;result={isError:false,content:[{type:'text',text:JSON.stringify(publicReceipt)}]};}
  }else result={};
  res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({jsonrpc:'2.0',id:request.id,result}));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 return {url:`http://127.0.0.1:${server.address().port}/mcp`,receipts,close:()=>new Promise(resolve=>server.close(resolve))};
}
