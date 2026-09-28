// 实际 HTTP/SSE 服务驱动网页重连，模拟慢客户端驱逐及窗口过期。
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const http=require('node:http');
(async()=>{
 const token='test-stream-token-123456789';
 let sequence=1,session='first',streams=new Set(),lost=false,rejectHandshake=false,requests=[],handshakes=[];
 const sample=n=>({schema_version:1,session_id:session,sequence:n,uptime_s:n,group:'system',complete:true,processes:[],samples:[{schema_version:1,session_id:session,sequence:n,uptime_s:n,timestamp_unix:null,metric:'cpu.usage',entity:'cpu',value:n,unit:'percent',kind:'gauge',status:'ok'}]});
 const event=(res,name,data)=>res.write('event: '+name+'\ndata: '+JSON.stringify(data)+'\n\n');
 const server=http.createServer((req,res)=>{
  const url=new URL(req.url,'http://local');
  requests.push(req.url);
  if(url.pathname==='/'){res.setHeader('Content-Type','text/html');return res.end(fs.readFileSync(path.join(__dirname,'../web/procface-web.html')));}
  if(req.headers.authorization!=='Bearer '+token){res.writeHead(401);return res.end('{}');}
  if(url.pathname==='/api/v1/stream'){
   res.writeHead(200,{'Content-Type':'text/event-stream','Cache-Control':'no-store'});streams.add(res);
   res.on('close',()=>streams.delete(res));event(res,'connected',{session_id:session,sequence});return;
  }
  res.setHeader('Content-Type','application/json');
  if(url.pathname==='/api/v1/capabilities')return res.end(JSON.stringify({session_id:session,api_version:1,wire_schema:"procface-compact-v1",allow_unsigned_frontend:true}));
  if(url.pathname==='/api/v1/frontend/handshake'){
   let body='';req.on('data',chunk=>body+=chunk);req.on('end',()=>{handshakes.push(JSON.parse(body));if(rejectHandshake){res.writeHead(403);res.end('{"error":"invalid_signature","recommended_frontend_version":"0.2.0"}');}else res.end('{"accepted":true,"wire_schema":"procface-compact-v1"}');});return;
  }
  if(url.pathname==='/api/v1/trace')return res.end('{"state":"idle"}');
  if(url.pathname==='/api/v1/series'){
   const after=Number(url.searchParams.get('after')||0);
   const batches=[];for(let n=Math.max(after+1,lost?sequence:1);n<=sequence;n++)batches.push(sample(n));
   return res.end(JSON.stringify({session_id:session,sequence,history_gap:lost&&after<sequence-1,batches,has_more:false}));
  }
  res.writeHead(404);res.end('{}');
 });
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const origin='http://127.0.0.1:'+server.address().port;
 const browser=await chromium.launch({channel:process.env.PROCFACE_BROWSER_CHANNEL||'msedge',headless:true});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 try{
  await page.goto(origin);await page.locator('#address').fill(origin);await page.locator('#token').fill(token);await page.locator('#connect').click();
  await page.waitForFunction(()=>lastSequence===1);
  sequence=2;for(const res of streams)event(res,'sample',sample(2));
  await page.waitForFunction(()=>lastSequence===2);
  // 服务端明确驱逐慢客户端，短窗口内补偿全部样本。
  sequence=4;for(const res of streams){event(res,'error',{error:'slow_client'});res.end();}
  await page.waitForFunction(()=>lastSequence===4&&gaps.some(g=>g.recovered),{},{timeout:10000});
  assert.equal(await page.evaluate(()=>gapCrosses(2,4)),false);
  // 下次驱逐时只剩窗口末尾，前端必须保留缺口。
  sequence=8;lost=true;for(const res of streams)res.destroy();
  await page.waitForFunction(()=>lastSequence===8&&gaps.some(g=>!g.recovered&&g.end_wall!==null),{},{timeout:10000});
  assert.equal(await page.evaluate(()=>gapCrosses(4,8)),true);
  assert(handshakes.length>=3);assert(handshakes.every(h=>h.frontend_version&&h.build_id&&h.api_compatibility));
  assert(!requests.some(url=>url.includes(token)));
  assert(!await page.evaluate(secret=>writeJson(memoryBatches).includes(secret),token));
  // daemon 在握手后换会话：实时事件必须触发重新协商，不能永远忽略新样本。
  session='second';sequence=1;lost=false;for(const res of streams)event(res,'sample',sample(1));
  await page.waitForFunction(()=>session==='second'&&lastSequence===1,{},{timeout:10000});
  assert(await page.evaluate(async()=> (await records('batches',base+'|first')).length>=5));
  await page.locator('#disconnect').click();rejectHandshake=true;requests=[];
  await page.locator('#connect').click();
  await page.waitForFunction(()=>document.querySelector('#message').textContent.includes('invalid_signature'));
  assert.match(await page.locator('#message').innerText(),/0\.2\.0/);
  assert.deepEqual(requests,['/api/v1/capabilities','/api/v1/frontend/handshake']);
  assert.deepEqual(errors,[]);
  console.log('前端实时流验证通过：HTTP 重连、慢客户端错误恢复、窗口内补偿、窗口外 gap、不跨缺口连线、重新握手与 daemon 会话切换。');
 }finally{await browser.close();for(const res of streams)res.end();server.closeAllConnections();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
