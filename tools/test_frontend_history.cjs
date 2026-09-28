// 用真实浏览器验证补偿与实时流的乱序交错，不需要 Linux daemon。
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const http=require('node:http');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const html=path.resolve(__dirname,'../web/procface-web.html');
(async()=>{
 const server=http.createServer((req,res)=>{res.setHeader('Content-Type','text/html');res.end(fs.readFileSync(html));});
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const browser=await chromium.launch({channel:process.env.PROCFACE_BROWSER_CHANNEL||'msedge',headless:true});
 const page=await browser.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 try{
  await page.goto(pathToFileURL(html).href);
  assert(await page.locator('#address').isVisible());assert(await page.locator('#token').isVisible());
  await page.goto('http://127.0.0.1:'+server.address().port);
  await page.waitForFunction(()=>db!==null);
  const requests=[];
  await page.route('http://device.test/**',route=>{
   requests.push(new URL(route.request().url()).pathname);
   return route.fulfill({json:{api_version:2,recommended_frontend_version:'2.0.0'}});
  });
  await page.locator('#address').fill('http://device.test');
  await page.locator('#token').fill('test-only-token');
  await page.locator('#connect').click();
  await page.waitForFunction(()=>document.querySelector('#message').textContent.includes('API 不兼容'));
  assert.match(await page.locator('#message').innerText(),/2\.0\.0/);
  assert.deepEqual(requests,['/api/v1/capabilities']);
  await page.unroute('http://device.test/**');
  // 实时更新不得替换 canvas、进程行或折叠正在查看的 Trace。
  await page.evaluate(()=>{
   resetView();
   const sample=(metric,entity='system')=>({metric,entity,value:10,uptime_s:1,sequence:1,unit:'bytes',status:'ok'});
   plotBatch({group:'system',samples:[...Array.from(defaultMetrics,m=>sample(m,m==='cpu.usage'?'cpu':'system')),sample('network.rx_bytes_per_second','eth1'),sample('vm.page_in_bytes')]});
   latestProcesses=[{identity:{pid:42,starttime_ticks:1},name:'stable',state:'R',cpu_percent:1,rss_bytes:100,threads:1}];
   render();
   const canvas=$('charts').querySelector('canvas'),row=$('processes').firstChild;
   if($('charts').children.length!==defaultMetrics.size+1)throw Error('默认指标或多设备覆盖不足');
   renderTraceFields({samples:[sample('trace.status')]});
   const detail=$('traceFields').firstChild;detail.open=true;
   render();renderTraceFields({samples:[sample('trace.status')]});
   if(canvas!==$('charts').querySelector('canvas')||row!==$('processes').firstChild||detail!==$('traceFields').firstChild||!detail.open)throw Error('实时刷新替换了可视节点');
   $('allCharts').click();render();
   if($('charts').children.length!==points.size)throw Error('全部指标未覆盖');
   resetView();
  });
  const result=await page.evaluate(async()=>{
   base='http://device.test';session='test';viewSession=base+'|'+session;
   function batch(n){return {session_id:session,sequence:n,uptime_s:n,group:'process',complete:true,processes:[{identity:{pid:n},name:'p'+n,state:'R',cpu_percent:n,rss_bytes:n,threads:1}],samples:[{session_id:session,sequence:n,uptime_s:n,metric:'cpu.usage',entity:'cpu',value:n,unit:'percent',status:'ok'}]};}
   await ingest(batch(3));await ingest(batch(1));await ingest(batch(2));await ingest(batch(3));
   const ordered=points.get('cpu.usage|cpu').map(p=>p.sequence),pid=latestProcesses[0].identity.pid;
   const persisted=await records('batches',viewSession);
   // 补偿缺少批次 4：批次 5 到达时必须留下缺口，不跨缺口连线。
   const fetchOriginal=window.fetch;
   window.fetch=async()=>new Response(JSON.stringify({history_gap:true,batches:[],has_more:false}));
   const encoder=new TextEncoder();
   const stream=new ReadableStream({start(c){c.enqueue(encoder.encode('event: sample\ndata: '+JSON.stringify(batch(5))+'\n\n'));c.close();}});
   try{await consume(new Response(stream),new AbortController().signal,generation);}catch(e){if(e.message!=='实时流已关闭')throw e;}
   const missing=gaps.some(g=>!g.recovered)&&gapCrosses(3,5);
   // 短断链可由 series 补全，不得把已补齐区间继续标红。
   beginGap();window.fetch=async()=>new Response(JSON.stringify({history_gap:false,batches:[batch(6),batch(7)],has_more:false}));
   await recover(generation,7);const recovered=gaps.at(-1).recovered;
   // 已更换连接的异步响应不能污染当前页面或存储。
   const old=generation;generation++;let aborted=false;
   try{await ingest(batch(8),old);}catch(e){aborted=e.name==='AbortError';}
   window.fetch=fetchOriginal;
   const precision=writeJson(parseJson('{"value":18446744073709551615}'));
   return {ordered,pid,count:persisted.length,missing,recovered,aborted,last:lastSequence,precision};
  });
  assert.deepEqual(result,{ordered:[1,2,3],pid:3,count:3,missing:true,recovered:true,aborted:true,last:7,precision:'{"value":18446744073709551615}'});
  // 完整进程列表留在采集记录中，排序与显示数量只改变页面。
  await page.evaluate(async()=>{
   const processes=Array.from({length:20},(_,i)=>({identity:{pid:100+i,starttime_ticks:123},name:'worker'+i,state:'R',cpu_percent:i,rss_bytes:1024*(20-i),threads:1}));
   await ingest({schema_version:1,session_id:session,sequence:8,uptime_s:8,group:'process',complete:true,processes,samples:[{schema_version:1,session_id:session,sequence:8,uptime_s:8,timestamp_unix:null,metric:'cpu.usage',entity:'cpu',value:42,unit:'percent',kind:'gauge',status:'ok'},{schema_version:1,session_id:session,sequence:8,uptime_s:8,timestamp_unix:null,metric:'test.counter',entity:'system',value:18446744073709551615n,unit:'bytes',kind:'counter',status:'ok'}]});
  });
  await page.waitForFunction(()=>document.querySelectorAll('#processes tr').length===16);
  assert.equal(await page.locator('#processes tr').first().locator('td').first().innerText(),'119');
  await page.locator('#sort').selectOption('memory');
  await page.waitForFunction(()=>document.querySelector('#processes td').textContent==='100');
  await page.locator('#top').selectOption('0');
  await page.waitForFunction(()=>document.querySelectorAll('#processes tr').length===20);
  await page.locator('#search').fill('worker19');
  await page.waitForFunction(()=>document.querySelectorAll('#processes tr').length===1);
  assert.equal(await page.evaluate(()=>memoryBatches.at(-1).batch.processes.length),20);
  await page.locator('#search').fill('');
  await page.evaluate(()=>{coverage=false;queueRender();});
  await page.waitForFunction(()=>document.querySelector('#processState').textContent.includes('覆盖不足'));
  await page.locator('#refreshHistory').click();
  await page.waitForFunction(()=>document.querySelector('#historySession').options.length>1);
  await page.locator('#historySession').selectOption('http://device.test|test');
  await page.locator('#loadHistory').click();
  await page.waitForFunction(()=>document.querySelector('#message').textContent.includes('回看历史'));
  assert.equal(await page.locator('#processes tr').count(),20);
  await page.locator('#live').click();
  assert.equal(await page.evaluate(()=>memoryBatches.at(-1).batch.processes.length),20);
  for(const format of ['json','csv','tsv']){
   await page.locator('#format').selectOption(format);
   const ready=page.waitForEvent('download');await page.locator('#export').click();
   const download=await ready,text=fs.readFileSync(await download.path(),'utf8');
   assert(text.includes('18446744073709551615'));assert(!text.includes('test-only-token'));
   if(format==='json'){const data=JSON.parse(text);assert.equal(data.batches.at(-1).processes.length,20);assert(data.gaps.some(g=>!g.recovered));}
   else {assert(text.startsWith('record_type'+(format==='csv'?',':'\t')+'schema_version'));assert(text.includes('procface.gap'));}
  }
  const pngReady=page.waitForEvent('download');await page.getByRole('button',{name:'PNG',exact:true}).first().click();
  const png=fs.readFileSync(await(await pngReady).path());assert.equal(png.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
  const saved=await page.evaluate(async()=>writeJson({batches:await records('batches',viewSession),gaps:await records('gaps',viewSession)}));
  assert(!saved.includes('test-only-token'));
  const prefixGap=await page.evaluate(async()=>{
   session='prefix';viewSession=base+'|'+session;lastSequence=0;lastUptime=0;pendingGap=null;resetView();
   const original=window.fetch;
   window.fetch=async()=>new Response(JSON.stringify({history_gap:true,has_more:false,batches:[10,11].map(n=>({session_id:session,sequence:n,uptime_s:n,group:'system',samples:[],processes:[],complete:true}))}));
   await recover(generation);window.fetch=original;
   return {end:gaps.at(-1).to_uptime,crosses:gapCrosses(10.1,11)};
  });
  assert.deepEqual(prefixGap,{end:10,crosses:false});
  await page.evaluate(()=>{
   connected=true;showTrace({state:'running',options:{pid:100},identity:{pid:100,starttime_ticks:123}});
   window.tracePolls=0;
   window.fetch=async(url,options)=>new Response(JSON.stringify({state:options.method==='DELETE'||++window.tracePolls<2?'stopping':'idle',options:{pid:100},identity:{pid:100,starttime_ticks:123}}));
  });
  assert.match(await page.locator('#traceState').innerText(),/启动标识 123/);
  await page.locator('#stopTrace').click();
  assert.equal(await page.locator('#startTrace').isDisabled(),true);
  await page.waitForFunction(()=>trace.state==='idle');
  assert.equal(await page.locator('#startTrace').isEnabled(),true);
  assert.deepEqual(errors,[]);
  console.log('前端历史验证通过：离线界面、API 不兼容拒绝、乱序去重、IndexedDB、缺口断线、补偿、旧连接隔离、u64 精度。');
 }finally{await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
