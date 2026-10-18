/**
 * A page that renders the graph view outside VS Code, for screenshots and for
 * measuring geometry.
 *
 * It imports the same `view.ts` markup and stylesheet the extension uses, so a
 * screenshot is of the real interface rather than of a second copy that has
 * quietly drifted. Animation time is frozen through `?testTime=` so two runs
 * produce the same pixels.
 */

import { contentSecurityPolicy, markup, styles } from './view';
import { darkPalette, lightPalette } from '../theme';
import type { GraphSnapshot, ThemeTokens } from './protocol';

export interface HarnessScenario {
  name: string;
  width: number;
  height: number;
  theme: 'dark' | 'light' | 'high-contrast';
  snapshot: GraphSnapshot | null;
  state?: 'loading' | 'empty' | 'error' | 'noprovider';
  message?: string;
  reducedMotion?: boolean;
  /** Freeze the animation clock at this progress, 0..1. */
  testTime?: number;
  search?: string;
}

const node = (
  id: string,
  level: GraphSnapshot['nodes'][number]['level'],
  title: string,
  subtitle: string,
  depth: number,
  options: { children?: boolean; folder?: string; relation?: 'definition' | 'call' | 'reference' } = {},
): GraphSnapshot['nodes'][number] => ({
  id,
  level,
  title,
  subtitle,
  depth,
  hasChildren: options.children ?? false,
  folded: options.folder !== undefined,
  location: { uri: `file:///demo/${subtitle.split(':')[0] ?? 'file'}`, line: 1, column: 0 },
  ...(options.relation ? { relation: options.relation } : {}),
});

/** The scenarios the handoff lists, with the sizes it lists them at. */
export const scenarios: HarnessScenario[] = [
  {
    name: 'graph-dark-compact',
    width: 280,
    height: 820,
    theme: 'dark',
    testTime: 1,
    snapshot: {
      scope: 'file',
      relation: 'follow',
      total: 6,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true }),
        node('f1', 'folder', 'src/editor', '3 个文件', 1, { children: true }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 2),
        node('c1', 'class', 'GraphController', 'class', 2, { children: true }),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 3, { relation: 'definition' }),
        node('v1', 'variable', 'project', 'Project · graph.rs:7', 3),
      ],
    },
  },
  {
    name: 'graph-dark-expanded',
    width: 460,
    height: 900,
    theme: 'dark',
    testTime: 1,
    snapshot: {
      scope: 'project',
      relation: 'callers',
      total: 6,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true }),
        node('f1', 'folder', 'src/editor', '3 个文件', 1, { children: true }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 2),
        node('c1', 'class', 'GraphController', 'class', 2, { children: true }),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 3, { relation: 'call' }),
        node('call1', 'call', 'render_graph()', 'render.rs:58', 1, { relation: 'call' }),
      ],
    },
  },
  {
    name: 'graph-light-expanded',
    width: 460,
    height: 900,
    theme: 'light',
    testTime: 1,
    snapshot: {
      scope: 'project',
      relation: 'follow',
      total: 4,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 1),
        node('c1', 'class', 'GraphController', 'class', 1, { children: true }),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 2),
      ],
    },
  },
  { name: 'graph-empty', width: 460, height: 900, theme: 'dark', snapshot: null, state: 'empty' },
  { name: 'graph-loading', width: 460, height: 900, theme: 'dark', snapshot: null, state: 'loading' },
  {
    name: 'graph-error',
    width: 460,
    height: 900,
    theme: 'dark',
    snapshot: null,
    state: 'error',
    message: '无法读取调用层级',
  },
  {
    name: 'graph-folded',
    width: 460,
    height: 900,
    theme: 'dark',
    testTime: 1,
    snapshot: {
      scope: 'file',
      relation: 'follow',
      total: 5,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true, folder: 'p' }),
        node('f1', 'file', 'graph.rs', 'src/editor/graph.rs', 1),
      ],
    },
  },
  {
    name: 'graph-search',
    width: 460,
    height: 900,
    theme: 'dark',
    testTime: 1,
    search: 'graph',
    snapshot: {
      scope: 'project',
      relation: 'follow',
      total: 3,
      nodes: [
        node('c1', 'class', 'GraphController', 'class', 0, { children: true }),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 1, { relation: 'definition' }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 0),
      ],
    },
  },
  {
    name: 'reduced-motion',
    width: 460,
    height: 900,
    theme: 'dark',
    testTime: 0.5,
    reducedMotion: true,
    snapshot: {
      scope: 'file',
      relation: 'callees',
      total: 3,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 1),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 2),
      ],
    },
  },
  {
    name: 'high-contrast',
    width: 460,
    height: 900,
    theme: 'high-contrast',
    testTime: 1,
    snapshot: {
      scope: 'file',
      relation: 'follow',
      total: 3,
      nodes: [
        node('p', 'project', 'TC-IDE', '~/src/tc-ide', 0, { children: true }),
        node('f2', 'file', 'graph.rs', 'src/editor/graph.rs', 1),
        node('fn1', 'function', 'build_graph()', 'graph.rs:42', 2),
      ],
    },
  },
];

export function tokensFor(theme: HarnessScenario['theme']): ThemeTokens {
  return theme === 'light' ? lightPalette : darkPalette;
}

/** The whole page for one scenario. */
export function harnessPage(scenario: HarnessScenario): string {
  const nonce = 'harness-nonce';
  const tokens = tokensFor(scenario.theme);
  const boot = {
    scenario,
    tokens,
  };
  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8" />
<meta http-equiv="Content-Security-Policy" content="${contentSecurityPolicy('file:', nonce)}" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>${scenario.name}</title>
<style nonce="${nonce}">${styles({ nonce })}</style>
<style nonce="${nonce}">
  html, body { width: 100%; height: 100%; }
</style>
</head>
<body data-reduced-motion="${scenario.reducedMotion === true}"
      data-high-contrast="${scenario.theme === 'high-contrast'}"
      data-frozen="true">
${markup()}
<script nonce="${nonce}">window.__TC_HARNESS__ = ${JSON.stringify(boot)};</script>
<script nonce="${nonce}" src="/harness.js"></script>
<script nonce="${nonce}" src="/webview.js"></script>
</body>
</html>`;
}

/** Look a scenario up by name. */
export function findScenario(name: string): HarnessScenario | undefined {
  return scenarios.find((scenario) => scenario.name === name);
}

export { darkPalette, lightPalette };
