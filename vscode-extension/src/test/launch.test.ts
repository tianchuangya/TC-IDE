/**
 * The development host configuration.
 *
 * Phase A asks for F5 to start the Extension Development Host. That is easy to
 * believe is working because nothing tests it: a launch file pointing at a
 * folder with no symbols, or at a task that does not exist, still opens a
 * window - it just shows an empty view, which looks like a bug in the
 * extension.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { join, resolve } from 'node:path';

// The tests run from the extension directory, which is what npm sets as cwd.
// `import.meta.dirname` would not compile to CommonJS, so the working directory
// is used instead and asserted below.
const ROOT = resolve(process.cwd());
assert_extension_root();

function assert_extension_root(): void {
  if (!existsSync(join(ROOT, 'package.json'))) {
    throw new Error(`tests are not running from the extension directory: ${ROOT}`);
  }
}
const read = (rel: string): unknown => JSON.parse(readFileSync(join(ROOT, rel), 'utf8'));

test('there is a launch configuration for the development host', () => {
  const launch = read('.vscode/launch.json') as {
    configurations: { name: string; type: string; preLaunchTask?: string; args?: string[] }[];
  };
  assert.ok(Array.isArray(launch.configurations));
  const host = launch.configurations.find((entry) => entry.type === 'extensionHost');
  assert.ok(host, 'no extensionHost configuration: F5 would do nothing');

  // The Host has to be told where the extension is, or it loads an empty window.
  const devPath = (host.args ?? []).find((arg) => arg.includes('--extensionDevelopmentPath'));
  assert.ok(devPath, 'the configuration does not pass --extensionDevelopmentPath');

  // And it has to open a folder that is worth looking at. A folder with no
  // language symbols shows the empty state, which reads as a broken view.
  const opened = (host.args ?? []).filter((arg) => !arg.startsWith('--'));
  assert.ok(opened.length > 0, 'the configuration opens no folder');
  const fixture = opened[0]?.replace('${workspaceFolder}/', '') ?? '';
  assert.ok(fixture.length > 0);
  assert.ok(
    existsSync(join(ROOT, fixture)),
    `the folder F5 opens does not exist: ${fixture}`,
  );
  assert.ok(
    existsSync(join(ROOT, fixture, 'src')),
    `the folder F5 opens has no sources to analyse: ${fixture}`,
  );
});

test('the task before launch exists and builds the entry the manifest names', () => {
  const launch = read('.vscode/launch.json') as {
    configurations: { type: string; preLaunchTask?: string }[];
  };
  const tasks = read('.vscode/tasks.json') as { tasks: { label: string; script?: string }[] };
  const labels = new Set(tasks.tasks.map((task) => task.label));

  for (const config of launch.configurations) {
    if (!config.preLaunchTask) {
      continue;
    }
    assert.ok(
      labels.has(config.preLaunchTask),
      `preLaunchTask ${config.preLaunchTask} is not a task`,
    );
    const task = tasks.tasks.find((entry) => entry.label === config.preLaunchTask);
    assert.ok(task?.script, 'the pre-launch task runs no npm script');
  }

  // The manifest points at a bundle; the build has to write exactly that path.
  const pkg = read('package.json') as { main: string; scripts: Record<string, string> };
  assert.equal(pkg.main, './dist/extension.js');
  assert.ok(pkg.scripts['build'], 'there is no build script');
});

test('the fixture has the shape the call hierarchy needs', () => {
  // A fixture with no calls would make the hierarchy look broken when it is
  // simply empty, so the file is checked for the things the view shows.
  const source = readFileSync(join(ROOT, 'fixtures', 'rust', 'src', 'lib.rs'), 'utf8');
  assert.match(source, /struct\s+\w+/, 'no struct to show as a class node');
  assert.match(source, /impl\s+\w+/, 'no impl block, so no methods');
  assert.match(source, /fn\s+\w+\([^)]*\)[^{]*\{/, 'no functions');
  // A method calling another method gives the callees direction something to
  // report.
  assert.match(source, /self\.\w+\(\)/, 'no method calling another method');
  // And a self-call gives the callers direction a cycle to walk.
  assert.match(source, /\b(\w+)\([^)]*\)[^;]*\n[\s\S]*\1\(/, 'no recursive call');
});
