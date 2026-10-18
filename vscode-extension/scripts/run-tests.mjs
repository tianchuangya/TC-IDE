/**
 * Run the compiled unit tests.
 *
 * `node --test "glob"` behaves differently between Node 20, 22 and 24: the
 * glob was experimental in 20, and a quoted pattern that expands in one shell
 * does not in another. This walks the directory itself and spawns the runner
 * with an explicit file list, so the result does not depend on the Node
 * version or on how the shell quoted the pattern.
 */

import { readdir } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';

const ROOT = resolve(import.meta.dirname, '..');
const TEST_DIR = join(ROOT, 'dist', 'modules', 'test');

async function collect(dir) {
  const files = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collect(full)));
    } else if (entry.name.endsWith('.test.js')) {
      files.push(full);
    }
  }
  return files.sort();
}

const files = await collect(TEST_DIR).catch(() => []);
if (files.length === 0) {
  console.error(`no compiled tests under ${TEST_DIR}; run the build first`);
  process.exit(1);
}

// The exit code is set through the process rather than by exiting from the
// callback: exiting inside it can land before the runner has flushed, and the
// script then reports success for a failed run.
const code = await new Promise((done) => {
  const child = spawn(process.execPath, ['--test', ...files], { stdio: 'inherit' });
  child.on('exit', (status) => done(status ?? 1));
  child.on('error', () => done(1));
});
process.exitCode = code;
