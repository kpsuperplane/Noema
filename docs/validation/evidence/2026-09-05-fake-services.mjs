import https from 'node:https';
import { readFile } from 'node:fs/promises';
import { randomBytes, createHash, X509Certificate } from 'node:crypto';
import { createRequire } from 'node:module';
const require=createRequire('/root/noema/apps/web/package.json');
const { WebSocketServer }=require('ws');
const root='/var/tmp/noema-suite-run-20260905';
export async function startFakeServices() {
  const cert=await readFile(root+'/fake-ca.pem');
  const pin=createHash('sha256').update(new X509Certificate(cert).publicKey.export({type:'spki',format:'der'})).digest('base64');
  const key=await readFile(root+'/fake-credentials/key.pem');
  const replies=[], receipts=[], codes=new Map(), tokens=new Map(), refreshTokens=new Map();
  let sequence=0;
  const secret=()=>randomBytes(24).toString('base64url');
  async function generate(body,send) {
    const reply=replies.shift() || {text:'Controlled request complete.'};
    const n=++sequence, id=`resp_fake_${n}`;
    receipts.push({kind:'model',number:n,model:body.model,tools:body.tools?.map(t=>t.name).filter(Boolean),apiTools:body.tools?.filter(t=>t.name?.startsWith('lookup')).map(t=>({name:t.name,description:t.description})),inputTypes:body.input?.map(i=>i.type||i.role)});
    let item;
    if(reply.tool || reply.toolLabel) {
      const tool=body.tools.find(t=>reply.toolLabel ? t.description?.includes(reply.toolLabel) : t.name===reply.tool||t.name===reply.tool.replaceAll('.','_'));
      if(!tool) throw new Error('Expected tool was not offered: '+reply.tool);
      item={type:'function_call',id:`fc_fake_${n}`,call_id:`call_fake_${n}`,name:tool.name,arguments:JSON.stringify(reply.arguments),status:'completed'};
    } else item={type:'message',id:`msg_fake_${n}`,role:'assistant',status:'completed',content:[{type:'output_text',text:reply.text,annotations:[]}]};
    send({type:'response.created',response:{id,status:'in_progress',output:[]}});
    if(reply.text) send({type:'response.output_text.delta',output_index:0,delta:reply.text});
    send({type:'response.output_item.done',output_index:0,item});
    send({type:'response.completed',response:{id,model:body.model,status:'completed',output:[item],usage:{input_tokens:20,output_tokens:10,total_tokens:30}}});
  }
  const server=https.createServer({cert,key},async(req,res)=>{
    const u=new URL(req.url,`https://${req.headers.host}`);
    if(u.pathname==='/v1/responses') {
      const parts=[]; for await(const chunk of req) parts.push(chunk);
      res.writeHead(200,{'content-type':'text/event-stream'});
      await generate(JSON.parse(Buffer.concat(parts).toString()),v=>res.write('data: '+JSON.stringify(v)+'\n\n')); res.end(); return;
    }
    if(u.pathname==='/o/oauth2/v2/auth') {
      const q=u.searchParams;
      if(q.get('code_challenge_method')!=='S256'||!q.get('state')) {res.writeHead(400);res.end();return;}
      const code=secret(); const account=q.get('fixture_account')||'account-one';
      codes.set(code,{challenge:q.get('code_challenge'),redirect:q.get('redirect_uri'),scope:q.get('scope'),account});
      const redirect=new URL(q.get('redirect_uri')); redirect.searchParams.set('state',q.get('state'));
      if(q.get('fixture_denied')==='true') redirect.searchParams.set('error','access_denied'); else redirect.searchParams.set('code',code);
      receipts.push({kind:'consent',account,denied:q.get('fixture_denied')==='true',scope:q.get('scope')});
      res.writeHead(302,{location:redirect.href});res.end();return;
    }
    if(u.pathname==='/token') {
      const parts=[];for await(const c of req) parts.push(c);const q=new URLSearchParams(Buffer.concat(parts).toString());
      let record;
      if(q.get('grant_type')==='authorization_code') {
        record=codes.get(q.get('code'));codes.delete(q.get('code'));
        if(!record||createHash('sha256').update(q.get('code_verifier')||'').digest('base64url')!==record.challenge||q.get('redirect_uri')!==record.redirect) record=null;
      } else record=refreshTokens.get(q.get('refresh_token'));
      res.setHeader('content-type','application/json');
      if(!record){res.writeHead(400);res.end(JSON.stringify({error:'invalid_grant'}));return;}
      const access=secret(),refresh=secret();tokens.set(access,record);refreshTokens.set(refresh,record);
      receipts.push({kind:'token',grantType:q.get('grant_type'),account:record.account,scope:record.scope});
      res.end(JSON.stringify({access_token:access,refresh_token:refresh,token_type:'Bearer',expires_in:3600,scope:record.scope}));return;
    }
    if(u.pathname.startsWith('/v1/items/')) {
      const record=tokens.get((req.headers.authorization||'').replace(/^Bearer /,''));
      if(!record){res.writeHead(401);res.end();return;}
      receipts.push({kind:'api-read',account:record.account,path:u.pathname});
      res.setHeader('content-type','application/json');res.end(JSON.stringify({name:record.account+' record'}));return;
    }
    res.writeHead(404);res.end();
  });
  const wss=new WebSocketServer({noServer:true});
  server.on('upgrade',(req,socket,head)=>{
    if(req.url!=='/v1/responses'){socket.destroy();return;}
    wss.handleUpgrade(req,socket,head,ws=>ws.on('message',async bytes=>{
      try {await generate(JSON.parse(bytes.toString()),v=>ws.send(JSON.stringify(v)));}
      catch(error){console.error(error.message);ws.close(1011);}
    }));
  });
  await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(443,'93.184.215.14',resolve);});
  return {replies,receipts,pin,expireAccess:()=>tokens.clear(),close:async()=>{for(const ws of wss.clients)ws.terminate();await new Promise(resolve=>server.close(resolve));}};
}
