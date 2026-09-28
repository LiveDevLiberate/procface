const {chromium}=require('playwright');
const fs=require('node:fs');
const assert=require('node:assert/strict');
(async()=>{
 const browser=await chromium.launch({channel:process.env.PROCFACE_BROWSER_CHANNEL||'msedge',headless:true});
 const context=await browser.newContext({viewport:{width:1280,height:900},colorScheme:'light',acceptDownloads:true});
 const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
 try{
  await page.goto(process.env.PROCFACE_WEB_URL||'http://127.0.0.1:8765/procface-web.html');
  await page.locator('#address').fill(process.env.PROCFACE_DAEMON_URL||'http://127.0.0.1:8766');
  await page.locator('#token').fill('test-token-01234567890123456789');
  await page.locator('#connect').click();
  await page.waitForFunction(()=>document.querySelector('#connection').textContent==='已连接',{},{timeout:15000});
  await page.waitForFunction(()=>document.querySelectorAll('#processes tr').length>0&&document.querySelectorAll('canvas').length>0);
  await page.waitForTimeout(1500);
  assert.equal(await page.locator('#debug').isVisible(),true);
  const target=page.locator('#processes tr').filter({hasText:'procface'}).first();
  if(await target.count())await target.getByRole('button').click();else await page.locator('#processes button').first().click();
  await page.locator('#startTrace').click();
  await page.waitForFunction(()=>document.querySelector('#traceState').textContent.includes('running'));
  await page.locator('#disconnect').click();
  await page.locator('#connect').click();
  await page.waitForFunction(()=>document.querySelector('#connection').textContent==='已连接'&&document.querySelector('#traceState').textContent.includes('running'));
  await page.locator('#stopTrace').click();
  await page.waitForFunction(()=>document.querySelector('#traceState').textContent.includes('idle'));
  const previous=Number(await page.locator('#pid').inputValue());
  const replacement=await page.evaluate(pid=>latestProcesses.find(p=>p.identity.pid!==pid)?.identity.pid,previous);
  assert(replacement,'切换测试需要两个可读进程');
  await page.locator('#pid').fill(String(replacement));
  await page.locator('#startTrace').click();
  await page.waitForFunction(pid=>trace.state==='running'&&trace.options.pid===pid,replacement);
  await page.locator('#stopTrace').click();
  await page.waitForFunction(()=>trace.state==='idle');
  const downloadPromise=page.waitForEvent('download');await page.locator('#export').click();const download=await downloadPromise;
  const path=await download.path(),data=JSON.parse(fs.readFileSync(path,'utf8'));assert(data.batches.length>0);assert(!JSON.stringify(data).includes('test-token-01234567890123456789'));
  assert.equal(errors.length,0,errors.join('\n'));
  fs.mkdirSync('docs',{recursive:true});await page.screenshot({path:'docs/procface-ui.png',fullPage:true});
  await page.locator('#disconnect').click();
  console.log('浏览器验证通过：连接、实时图表、进程列表、trace 重连接管、停止及历史导出；无页面异常。');
 }finally{await browser.close();}
})().catch(error=>{console.error(error);process.exitCode=1;});

