import * as esbuild from 'esbuild';

const watch = process.argv.includes('--watch');

// Two bundles: the extension entry the Extension Host loads, and the files the
// webview loads. The webview bundle is plain browser script, so it must not
// pull in anything from `vscode`.
const shared = {
  bundle: true,
  sourcemap: true,
  target: 'ES2022',
  logLevel: 'info',
  // Identifiers are kept: a minified Extension Host bundle turns every stack
  // trace from a user's report into an unreadable one, and the size saved is
  // not worth that.
  minify: false,
  keepNames: true,
};

const extension = {
  ...shared,
  entryPoints: ['src/extension.ts'],
  outfile: 'dist/extension.js',
  platform: 'node',
  format: 'cjs',
  external: ['vscode'],
};

const webview = {
  ...shared,
  entryPoints: ['src/graph/webviewMain.ts'],
  outfile: 'dist/webview.js',
  platform: 'browser',
  format: 'iife',
};

if (watch) {
  const contexts = await Promise.all([esbuild.context(extension), esbuild.context(webview)]);
  await Promise.all(contexts.map((context) => context.watch()));
  console.log('watching');
} else {
  await Promise.all([esbuild.build(extension), esbuild.build(webview)]);
}
