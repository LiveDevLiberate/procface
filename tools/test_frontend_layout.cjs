// 三页只切换展示；验证采集历史、Trace 状态和节点身份不会被切页重置。
const {chromium}=require('playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const http=require('node:http');
(async()=>{
 const server=http.createServer((req,res)=>res.end(fs.readFileSync('web/procface-web.html')));
 await new Promise(r=>server.listen(0,'127.0.0.1',r));
 const browser=await chromium.launch({channel:process.env.PROCFACE_BROWSER_CHANNEL||'msedge',headless:true});
 try{
  const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];
  page.on('pageerror',e=>errors.push(e.message));
  await page.goto('http://127.0.0.1:'+server.address().port);
  await page.waitForFunction(()=>db!==null);
  await page.evaluate(async()=>{
   base='http://192.0.2.1:9387';session='layout';viewSession=base+'|'+session;connected=true;
   $('connectionConfig').open=false;$('device').textContent='设备：'+base;
   $('intervalState').textContent='采样间隔：1 s';$('connection').textContent='已连接';
   showTrace({state:'running',options:{pid:42},identity:{pid:42,starttime_ticks:123}});
   window.fixtureBatch=(sequence,group,metrics)=>({session_id:session,sequence,uptime_s:sequence,group,complete:true,processes:[],samples:metrics.map(([metric,entity,value,unit])=>({session_id:session,sequence,uptime_s:sequence,metric,entity,value,unit,kind:'gauge',status:'ok'}))});
   for(let n=1;n<=3;n++)await ingest(fixtureBatch(n,'system',[
    ['cpu.usage','cpu',n*10,'percent'],['load.1m','system',n/10,'tasks'],
    ['memory.available_bytes','system',1024*1024*n,'bytes'],
    ['disk.read_bytes_per_second','sda',n*1024,'bytes/second'],
    ['network.rx_bytes_per_second','eth0',n*2048,'bytes/second']]));
   await ingest({...fixtureBatch(4,'process',[]),processes:[{identity:{pid:42,starttime_ticks:123},name:'worker',state:'R',rss_bytes:4096,threads:2,cpu_percent:3}]});
   await ingest(fixtureBatch(5,'trace',[
    ['process.cpu_usage','process:42:123',5,'percent'],['process.rss_bytes','process:42:123',4096,'bytes'],
    ['trace.status','process:42:123',{name:'worker',threads:2},'text']]));
   render();window.originalCanvas=$('charts').querySelector('canvas');window.originalRow=$('processes').firstChild;
   window.originalDetail=$('traceFields').lastChild;originalDetail.open=true;
  });
  assert.deepEqual(await page.locator('#charts h2').allTextContents(),['CPU','负载','内存','磁盘','网络']);
  assert.equal(await page.locator('#tab-process').isVisible(),false);
  await page.getByRole('tab',{name:'总览',exact:true}).focus();
  await page.keyboard.press('ArrowRight');
  assert.equal(await page.getByRole('tab',{name:'进程',exact:true}).getAttribute('aria-selected'),'true');
  assert.equal(await page.locator('#tab-process').isVisible(),true);
  await page.getByRole('button',{name:'选择 Trace',exact:true}).click();
  assert.equal(await page.locator('#pid').inputValue(),'42');
  assert.equal(await page.locator('#tab-trace').isVisible(),true);
  assert.equal(await page.locator('#traceCharts canvas').count(),2);
  assert.match(await page.locator('#traceFields pre').nth(2).textContent(),/"threads":2/);
  await page.evaluate(async()=>{
   await ingest(fixtureBatch(6,'system',[['cpu.usage','cpu',55,'percent']]));
   await ingest(fixtureBatch(7,'trace',[['trace.status','process:42:123',{threads:3},'text']]));
  });
  for(const name of ['总览','进程','Trace','总览'])await page.getByRole('tab',{name,exact:true}).click();
  const state=await page.evaluate(async()=>({count:(await records('batches',viewSession)).length,trace:trace.state,
   canvas:originalCanvas===$('charts').querySelector('canvas'),row:originalRow===$('processes').firstChild,
   detail:originalDetail===$('traceFields').firstChild,open:originalDetail.open,
   right:getComputedStyle(originalRow.cells[3]).textAlign,radius:getComputedStyle($('connect')).borderRadius}));
  assert.deepEqual(state,{count:7,trace:'running',canvas:true,row:true,detail:true,open:true,right:'right',radius:'0px'});
  assert.deepEqual(errors,[]);
  if(process.env.PROCFACE_SCREENSHOT_DIR){fs.mkdirSync(process.env.PROCFACE_SCREENSHOT_DIR,{recursive:true});for(const name of ['overview','process','trace']){await page.locator('#label-'+name).click();await page.screenshot({path:process.env.PROCFACE_SCREENSHOT_DIR+'/'+name+'.png',fullPage:true});}}
  console.log('三页布局验证通过：分组、键盘切页、Trace 曲线、节点稳定、历史持续保存、数值列对齐。');
 }finally{await browser.close();await new Promise(r=>server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
