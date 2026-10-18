/**
 * Generate the harness pages and, when a browser is available, the screenshots.
 *
 * The pages are written to `dist/harness/` and are plain files: opening one in
 * a browser shows the real view at a fixed size with time frozen. Producing
 * PNGs needs Chromium, which is not assumed; the geometry measurements run
 * against the same pages with a headless browser when one is found, and the
 * script says plainly when it is not, instead of silently skipping a check.
 */

import { createServer } from 'node:http';
import { copyFile, readFile, mkdir, writeFile } from 'node:fs/promises';
import { extname, join, resolve } from 'node:path';

import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { findScenario, harnessPage, scenarios } = require('../dist/graph/harness.js');

const ROOT = resolve(import.meta.dirname, '..');
const OUT = join(ROOT, 'dist', 'harness');

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
};

async function writePages() {
  await mkdir(OUT, { recursive: true });
  // Copy the pieces the pages load, so a capture always runs the current
  // bundle and the current harness shim rather than whatever was there last.
  for (const [from, to] of [
    [join(ROOT, 'dist', 'webview.js'), join(OUT, 'webview.js')],
    [join(ROOT, 'media', 'harness.js'), join(OUT, 'harness.js')],
  ]) {
    await copyFile(from, to);
  }
  for (const scenario of scenarios) {
    const page = harnessPage(scenario);
    await writeFile(join(OUT, `${scenario.name}.html`), page, 'utf8');
  }
  console.log(`wrote ${scenarios.length} harness pages to dist/harness`);
}

/** Serve the pages and the bundle, so a browser can load them. */
function serve(port) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? '/', `http://localhost:${port}`);
    const wanted = url.pathname === '/' ? '/graph-dark-compact.html' : url.pathname;
    const file = join(OUT, wanted.replace(/^\//, ''));
    try {
      const body = await readFile(file);
      response.writeHead(200, { 'content-type': MIME[extname(file)] ?? 'application/octet-stream' });
      response.end(body);
    } catch {
      // The bundle lives one directory up from the pages.
      try {
        const shared = await readFile(join(ROOT, 'dist', wanted.replace(/^\//, '')));
        response.writeHead(200, { 'content-type': MIME[extname(wanted)] ?? 'application/octet-stream' });
        response.end(shared);
      } catch {
        response.writeHead(404).end('not found');
      }
    }
  });
  return new Promise((resolvePromise) => {
    server.listen(port, '127.0.0.1', () => {
      resolvePromise({
        url: `http://127.0.0.1:${port}`,
        close: () =>
          new Promise((done) => {
            server.close(() => done());
          }),
      });
    });
  });
}

async function main() {
  const args = process.argv.slice(2);
  const name = args.find((arg) => !arg.startsWith('--'));
  await writePages();

  if (args.includes('--serve')) {
    const port = 7788;
    const { url, close } = await serve(port);
    console.log(`serving the harness at ${url}`);
    console.log('press Ctrl+C to stop');
    process.on('SIGINT', () => {
      void close().then(() => process.exit(0));
    });
    return;
  }

  if (name) {
    const scenario = findScenario(name);
    if (!scenario) {
      console.error(`unknown scenario: ${name}`);
      console.error(`known: ${scenarios.map((entry) => entry.name).join(', ')}`);
      process.exitCode = 1;
      return;
    }
    console.log(`page: dist/harness/${scenario.name}.html`);
    return;
  }

  // No browser is bundled with the repository, so a screenshot run says what
  // it needs rather than pretending it passed.
  console.log('PNG capture needs a Chromium binary; pass --serve and open the');
  console.log('pages, or set TC_IDE_CHROMIUM to the browser path.');
  const chromium = process.env['TC_IDE_CHROMIUM'];
  if (!chromium) {
    console.log('TC_IDE_CHROMIUM is not set, so no screenshots were taken.');
    console.log('The pages above are the ones to capture; the first baseline must');
    console.log('be approved by a person before it is used as a regression target.');
    return;
  }
  console.log(`using browser: ${chromium}`);
  const port = 7789;
  const { url } = await serve(port);
  await mkdir(join(ROOT, 'harness-shots'), { recursive: true });

  // Puppeteer is used rather than the browser's own --screenshot flag: that
  // flag fires before the page's messages arrive, so every state rendered as
  // the initial empty view and two scenarios came out byte identical.
  const { createRequire } = await import('node:module');
  const require = createRequire(import.meta.url);
  const puppeteer = require('puppeteer');
  const browser = await puppeteer.launch({
    executablePath: chromium,
    args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  });
  for (const scenario of scenarios) {
    const page = await browser.newPage();
    await page.setViewport({ width: scenario.width, height: scenario.height, deviceScaleFactor: 1 });
    await page.goto(`${url}/${scenario.name}.html`, { waitUntil: 'load' });
    // The harness delivers its messages on the next tick; wait for the flag it
    // sets rather than guessing a delay.
    await page.waitForFunction('window.__TC_HARNESS_READY__ === true', { timeout: 5000 }).catch(() => {
      console.error(`harness did not become ready for ${scenario.name}`);
    });
    await page.screenshot({ path: join(ROOT, 'harness-shots', `${scenario.name}.png`) });
    await page.close();
    console.log(`shot ${scenario.name}`);
  }
  await browser.close();
}

await main();
