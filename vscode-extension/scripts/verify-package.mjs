/**
 * Check the packaged extension before it is called done.
 *
 * A `.vsix` that installs but does nothing is the failure this guards against:
 * it lists what must be inside the archive, and refuses a package whose
 * manifest points at files that are not in it.
 */

import { createRequire } from 'node:module';
import { readFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const require = createRequire(import.meta.url);
const run = promisify(execFile);

const ROOT = resolve(import.meta.dirname, '..');

const problems = [];
function require_ok(condition, message) {
  if (!condition) {
    problems.push(message);
  }
}

async function main() {
  const manifest = JSON.parse(await readFile(join(ROOT, 'package.json'), 'utf8'));

  // The manifest has to point at things that exist.
  const must = [
    ['main', manifest.main],
    ['l10n', manifest.l10n],
  ];
  for (const [label, rel] of must) {
    require_ok(typeof rel === 'string' && rel.length > 0, `${label} is missing`);
  }
  const files = [
    manifest.main,
    'dist/webview.js',
    'media/icons/tc-ide.svg',
    'media/injected/tc-motion.css',
    'media/injected/tc-motion.js',
    'l10n/bundle.l10n.json',
    'l10n/bundle.l10n.zh-cn.json',
    'themes/tc-ide-dark-color-theme.json',
    'themes/tc-ide-light-color-theme.json',
    'README.md',
    'LICENSE',
  ];
  for (const rel of files) {
    if (typeof rel !== 'string') {
      continue;
    }
    try {
      const info = await stat(join(ROOT, rel));
      require_ok(info.size > 0, `${rel} is empty`);
    } catch {
      problems.push(`${rel} is missing from the package`);
    }
  }

  // Both themes must parse and carry real colours, so switching does not fall
  // back to the default theme silently.
  for (const theme of manifest.contributes?.themes ?? []) {
    try {
      const parsed = JSON.parse(await readFile(join(ROOT, theme.path.replace(/^\.\//, '')), 'utf8'));
      require_ok(parsed.colors && Object.keys(parsed.colors).length > 20, `${theme.path} has too few colours`);
      require_ok(Array.isArray(parsed.tokenColors) && parsed.tokenColors.length > 5, `${theme.path} has too few token colours`);
    } catch (error) {
      problems.push(`${theme.path} does not parse: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  // Every command the manifest offers has to be registered in the built bundle.
  const bundle = await readFile(join(ROOT, 'dist', 'extension.js'), 'utf8');
  for (const command of manifest.contributes?.commands ?? []) {
    require_ok(
      bundle.includes(command.command),
      `command ${command.command} is offered but not registered in the bundle`,
    );
  }

  // A webview view has to be backed by a provider for the same id. The bundler
  // may split a dotted literal, so the check looks for the parts within a short
  // distance of each other rather than for the exact string.
  const views = Object.values(manifest.contributes?.views ?? {}).flat();
  for (const view of views) {
    const [head, tail] = String(view.id).split('.');
    const joined = new RegExp(`${head}["']\\s*\\+\\s*["']\\.${tail}|${head}\\.${tail}`);
    require_ok(
      joined.test(bundle) || bundle.includes(view.id),
      `view ${view.id} is offered but no provider names it`,
    );
  }

  // No secrets in the packaged sources.
  const secretPatterns = [/(api[_-]?key|secret|token)\s*[:=]\s*['"][A-Za-z0-9_-]{16,}/i, /sk-[A-Za-z0-9]{20,}/];
  for (const rel of ['dist/extension.js', 'dist/webview.js']) {
    const text = await readFile(join(ROOT, rel), 'utf8');
    for (const pattern of secretPatterns) {
      require_ok(!pattern.test(text), `${rel} looks like it contains a secret`);
    }
  }

  // Build the archive when the packager is available, and check it exists.
  if (process.argv.includes('--build')) {
    try {
      await run('npx', ['vsce', 'package', '--no-dependencies', '--out', 'tc-ide-companion.vsix'], {
        cwd: ROOT,
        timeout: 300000,
      });
      const info = await stat(join(ROOT, 'tc-ide-companion.vsix'));
      require_ok(info.size > 1024, 'the vsix is suspiciously small');
      console.log(`vsix built: ${info.size} bytes`);
    } catch (error) {
      problems.push(`packaging failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  if (problems.length > 0) {
    console.error('package check failed:');
    for (const problem of problems) {
      console.error(`  - ${problem}`);
    }
    process.exitCode = 1;
    return;
  }
  console.log(`package check passed (${files.length} files, ${manifest.contributes?.commands?.length ?? 0} commands)`);
}

await main();
