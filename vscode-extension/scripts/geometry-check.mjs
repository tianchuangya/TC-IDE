/**
 * Measure the rendered view in a real browser and assert the handoff's numbers.
 *
 * This is the check that makes "does it look right" answerable without eyes.
 * Every assertion names the tolerance the handoff allows, and reads the DOM the
 * same page a person would see.
 */

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, resolve } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const puppeteer = require('puppeteer');

const ROOT = resolve(import.meta.dirname, '..');
const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
};

const SCENARIOS = [
  'graph-dark-compact',
  'graph-dark-expanded',
  'graph-light-expanded',
  'graph-empty',
  'graph-loading',
  'graph-error',
  'graph-folded',
  'graph-search',
  'reduced-motion',
  'high-contrast',
];

function serve(root) {
  const server = createServer(async (request, response) => {
    const url = new URL(request.url ?? '/', 'http://x');
    const file = join(root, url.pathname.replace(/^\//, ''));
    try {
      const body = await readFile(file);
      response.writeHead(200, { 'content-type': MIME[extname(file)] ?? 'application/octet-stream' });
      response.end(body);
    } catch {
      response.writeHead(404).end('not found');
    }
  });
  return new Promise((done) => {
    server.listen(0, '127.0.0.1', () => {
      done({ server, port: server.address().port });
    });
  });
}

const results = [];
function check(name, ok, detail) {
  results.push({ name, ok, detail });
  if (!ok) {
    console.log(`  FAIL ${name}${detail ? `: ${detail}` : ''}`);
  }
}

async function measure(page, scenario) {
  return page.evaluate(() => {
    const rect = (selector) => {
      const found = document.querySelector(selector);
      return found ? found.getBoundingClientRect().toJSON() : null;
    };
    const cards = [...document.querySelectorAll('.card')].map((card) => ({
      left: card.getBoundingClientRect().left,
      top: card.getBoundingClientRect().top,
      height: card.getBoundingClientRect().height,
      hasFold: card.querySelector('.fold') !== null,
      level: Number.parseFloat(card.getAttribute('aria-level') ?? '1'),
    }));
    return {
      scrollWidth: document.documentElement.scrollWidth,
      clientWidth: document.documentElement.clientWidth,
      title: rect('.title-row'),
      rail: rect('.rail'),
      search: rect('#search-row:not([hidden])'),
      breadcrumb: rect('.breadcrumb'),
      viewport: rect('.viewport'),
      footer: rect('.footer'),
      indicator: rect('.rail-indicator'),
      cards,
      // A hidden control has no hit box to check; requiring one would fail
      // the clear button whenever the search box is empty, which is correct.
      iconButtons: [...document.querySelectorAll('.icon-button')]
        .filter((button) => button.getBoundingClientRect().width > 0)
        .map((button) => button.getBoundingClientRect().toJSON()),
    };
  });
}

async function run() {
  // The pages and the bundles are generated here rather than assumed: a fresh
  // checkout has no dist/harness, and reading it anyway made every asset 404.
  const { execFile } = await import('node:child_process');
  const { promisify } = await import('node:util');
  const run_ = promisify(execFile);
  await run_('node', ['scripts/capture-harness.mjs'], { cwd: ROOT });

  const { server, port } = await serve(join(ROOT, 'dist', 'harness'));
  const browser = await puppeteer.launch({
    args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  });

  const sizes = {
    'graph-dark-compact': [280, 820],
    'graph-dark-expanded': [460, 900],
    'graph-light-expanded': [460, 900],
  };

  for (const name of SCENARIOS) {
    const [width, height] = sizes[name] ?? [460, 900];
    const page = await browser.newPage();
    await page.setViewport({ width, height, deviceScaleFactor: 1 });
    const pageErrors = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    page.on('console', (message) => {
      if (message.type() === 'error') {
        pageErrors.push(message.text());
      }
    });

    await page.goto(`http://127.0.0.1:${port}/${name}.html`, { waitUntil: 'load' });
    // The harness delivers its messages on the next tick.
    await new Promise((done) => setTimeout(done, 400));

    check(`${name}: no page errors`, pageErrors.length === 0, pageErrors.join(' | '));

    const m = await measure(page, name);

    // No horizontal page scrollbar, in any mode.
    check(
      `${name}: no horizontal overflow`,
      m.scrollWidth <= m.clientWidth + 1,
      `${m.scrollWidth} > ${m.clientWidth}`,
    );

    // The rows, within the handoff's +/-1px.
    if (m.title) {
      check(`${name}: title row is 32±1`, Math.abs(m.title.height - 32) <= 1, String(m.title.height));
    }
    if (m.rail) {
      check(`${name}: rail is 32±1`, Math.abs(m.rail.height - 32) <= 1, String(m.rail.height));
    }
    if (m.footer) {
      check(`${name}: footer is 34±1`, Math.abs(m.footer.height - 34) <= 1, String(m.footer.height));
    }
    if (m.breadcrumb) {
      check(
        `${name}: breadcrumb is 24±1`,
        Math.abs(m.breadcrumb.height - 24) <= 1,
        String(m.breadcrumb.height),
      );
    }

    // Icon hit boxes are at least 28x28.
    for (const box of m.iconButtons) {
      check(
        `${name}: icon hit box ≥28`,
        box.width >= 28 && box.height >= 28,
        `${box.width}x${box.height}`,
      );
    }

    // The indicator is 2+/-0.5 tall.
    if (m.indicator && m.indicator.width > 0) {
      check(
        `${name}: indicator is 2±0.5`,
        Math.abs(m.indicator.height - 2) <= 0.5,
        String(m.indicator.height),
      );
    }

    // Card geometry: the first level at 42+/-1, each depth 16+/-1 apart,
    // slots 58+/-1 apart, cards 46 high.
    if (m.cards.length > 0) {
      const first = m.cards[0];
      check(`${name}: first card at 42±1`, Math.abs(first.left - 42) <= 1, String(first.left));
      const byLevel = new Map();
      for (const card of m.cards) {
        const list = byLevel.get(card.level) ?? [];
        list.push(card);
        byLevel.set(card.level, list);
      }
      for (const [level, cards] of byLevel) {
        for (const card of cards) {
          const expected = 42 + (level - 1) * 16;
          check(
            `${name}: level ${level} card at ${expected}±1`,
            Math.abs(card.left - expected) <= 1,
            String(card.left),
          );
        }
      }
      for (let index = 1; index < m.cards.length; index += 1) {
        const gap = m.cards[index].top - m.cards[index - 1].top;
        check(`${name}: row pitch 58±1`, Math.abs(gap - 58) <= 1 || gap <= 0, String(gap));
      }
      for (const card of m.cards) {
        check(`${name}: card height 46±1`, Math.abs(card.height - 46) <= 1, String(card.height));
      }
    }

    // Only nodes with children get a fold control.
    if (name === 'graph-dark-expanded') {
      const withFold = m.cards.filter((card) => card.hasFold).length;
      check(`${name}: some nodes have a fold control`, withFold > 0, String(withFold));
      check(`${name}: some nodes do not`, withFold < m.cards.length, String(withFold));
    }

    // The state panels are exclusive.
    if (name === 'graph-empty' || name === 'graph-loading' || name === 'graph-error') {
      const visible = await page.evaluate(() => {
        const ids = ['state-loading', 'state-empty', 'state-error', 'state-noprovider'];
        return ids.filter((id) => {
          const found = document.getElementById(id);
          if (!found) {
            return false;
          }
          // Computed display, not just the attribute: a panel made visible by
          // a later render can still carry no hidden attribute.
          return getComputedStyle(found).display !== 'none';
        });
      });
      check(`${name}: exactly one state panel`, visible.length === 1, visible.join(','));
    }

    await page.close();
  }

  await browser.close();
  await new Promise((done) => server.close(done));

  const failed = results.filter((entry) => !entry.ok);
  console.log(`\n${results.length - failed.length}/${results.length} geometry assertions passed`);
  if (failed.length > 0) {
    console.log(`${failed.length} failed`);
    process.exitCode = 1;
  }
}

await run();
