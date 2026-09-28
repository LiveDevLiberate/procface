// 真实浏览器检查紧凑数据展开与 IndexedDB，而非仅用旧对象 mock。
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const http=require('node:http');
(async()=>{
 const server=http.createServer((req,res)=>res.end(fs.readFileSync('web/procface-web.html')));
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const browser=await chromium.launch({channel:process.env.PROCFACE_BROWSER_CHANNEL||'msedge',headless:true});
 try{
  const page=await browser.newPage();
  await page.goto('http://127.0.0.1:'+server.address().port);
  await page.waitForFunction(()=>db!==null);
  const result=await page.evaluate(async()=>{
   base='http://test';session='compact';viewSession=base+'|'+session;
   const names=['process.pid','process.name','process.state','process.rss_bytes','process.threads','process.cpu_seconds','process.cpu_usage'];
   const units=['id','text','text','bytes','threads','seconds','percent'];
   const metrics=names.map((name,i)=>[i+1,name,units[i],i===5?'counter':'gauge']);
   metrics.push([8,'trace.io','text','gauge']);
   const raw=[1,10,null,2,false,['权限不足'],{metrics,entities:[[1,'process:42:18446744073709551615']]},[[8,1,{counter:18446744073709551615n,list:[true,'文本',null]},2]],[[1,42,18446744073709551615n,'worker','R',1000,4096,2,1.5,null,127,5]]];
   const b=decodeWire({wire_schema:'procface-compact-v1',session_id:session,batches:[raw]})[0];
   structuredClone(b); // 含函数等不可持久化值时立即失败。
   await ingest(b);
   const saved=(await records('batches',viewSession))[0].batch;
   compactMetrics.clear();compactEntities.clear();
   const independent=decodeWire({wire_schema:'procface-compact-v1',session_id:session,batch:raw})[0];
   return {same:writeJson(saved)===writeJson(independent),count:saved.samples.length,statuses:saved.samples.map(s=>s.status),pid:saved.processes[0].identity.pid,complete:saved.complete,diagnostics:saved.diagnostics,integer:String(saved.processes[0].identity.starttime_ticks),trace:writeJson(saved.samples[0].value),warning:dbWarned};
  });
  assert.equal(result.same,true);assert.equal(result.count,8);assert.equal(result.pid,42);
  assert.equal(result.statuses[0],'permission_denied');assert.equal(result.statuses.at(-1),'stale');
  assert.equal(result.complete,false);assert.deepEqual(result.diagnostics,['权限不足']);
  assert.equal(result.integer,'18446744073709551615');
  assert.equal(result.trace,'{"counter":18446744073709551615,"list":[true,"文本",null]}');
  assert.equal(result.warning,false);
  console.log('紧凑前端验证通过：进程去重恢复、状态、u64、结构化 Trace、真实 IndexedDB 与独立解码。');
 }finally{await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
