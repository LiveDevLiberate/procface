// 验证静态子系统图不请求 daemon 架构接口，并可离线切换和操作。
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const http = require('node:http');

(async () => {
  const server = http.createServer((req, res) => {
    if (req.url !== '/') { res.writeHead(204); return res.end(); }
    res.end(fs.readFileSync('web/procface-web.html'));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ channel: process.env.PROCFACE_BROWSER_CHANNEL || 'msedge', headless: true });
  try {
    const page = await browser.newPage();
    page.setDefaultTimeout(5000);
    const errors = [], requests = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', request => requests.push(new URL(request.url()).pathname));
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForSelector('#topologyOverview .topology-card');
    assert.equal(await page.locator('#topologyOverview .topology-card').count(), 7);
    await page.selectOption('#language', 'en');
    await page.evaluate(() => {
      points.set('network.rx_bytes_per_second|eth0', [{ x: 1, y: 10240, unit: 'bytes/second', status: 'ok', sequence: 1 }]);
      points.set('network.tx_bytes_per_second|eth0', [{ x: 1, y: 2048, unit: 'bytes/second', status: 'ok', sequence: 1 }]);
      queueRender();
    });
    await page.waitForFunction(() => document.querySelector('#topologyOverview').textContent.includes('10.00 KiB/s'));
    await page.getByRole('button', { name: /Networking/ }).click();
    assert.equal(await page.locator('#topology svg').getAttribute('aria-label'), 'Networking');
    assert.equal(await page.locator('#topology .topology-node').count(), 4);
    assert.equal(await page.locator('#topology .topology-edge').count(), 3);
    await page.locator('#topology .topology-node').nth(3).press('Enter');
    await page.waitForFunction(() => document.querySelectorAll('#charts .chart').length >= 1);
    assert.equal(await page.locator('#topologyDetail').isVisible(), true);
    await page.locator('#topologyBack').click();
    assert.equal(await page.locator('#topologyOverview').isVisible(), true);
    for (const name of ['Scheduler', 'Memory', 'VFS / Filesystem', 'Block I/O', 'Networking', 'IRQ / SoftIRQ', 'Processes / Threads']) {
      await page.getByRole('button', { name: new RegExp(name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')) }).click();
      assert.equal(await page.locator('#topology .topology-node').count(), 4);
      assert.equal(await page.locator('#topology .topology-edge').count(), 3);
      await page.locator('#topologyBack').click();
    }
    assert.deepEqual(requests.filter(path => path.includes('architecture')), []);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
  console.log('前端子系统数据流图验证通过：七个子系统概览、独立组件图、键盘交互和无 daemon 架构请求。');
})().catch(error => { console.error(error); process.exitCode = 1; });
