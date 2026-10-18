/**
 * The graph webview's runtime.
 *
 * This runs inside the webview document, so it cannot import `vscode` and must
 * treat everything it receives as untrusted. It draws only the rows in the
 * visible window, which is what keeps a large project from costing a frame per
 * node.
 */

import { cardLeft, graphGeometry, metrics, pathX, rowTop, visibleWindow, widthModeFor } from './geometry';
import {
  parseNode,
  parseSnapshot,
  RELATIONS,
  SCOPES,
  type GraphNodeDTO,
  type GraphRelation,
  type GraphScope,
  type GraphSnapshot,
  type GraphToHost,
  type HostToGraph,
  type ThemeTokens,
} from './protocol';
import { REDUCED_MOTION_MS, newSpring, stepSpring } from '../motion';
import { levelColor, levelMarker, relationLabel } from '../theme';

/** The API the webview is handed by VS Code. */
interface VsCodeApi {
  postMessage(message: GraphToHost): void;
  getState(): unknown;
  setState(state: unknown): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

const vscode = acquireVsCodeApi();

/** The root's horizontal padding, which every child is positioned inside. */
const ROOT_PADDING = metrics.rootPaddingX;

const RELATION_LABELS: Record<GraphRelation, string> = {
  follow: '跟随',
  callers: '调用者',
  callees: '被调用',
};

const SCOPE_LABELS: Record<GraphScope, string> = {
  auto: '自动',
  project: '整个项目',
  folder: '当前文件夹',
  file: '当前文件',
  symbol: '当前符号',
};

interface ViewState {
  graph: GraphSnapshot;
  tokens: ThemeTokens | null;
  reducedMotion: boolean;
  selectedId: string | null;
  query: string;
  /** Folded node ids, kept here so a redraw does not lose them. */
  folded: Set<string>;
  /** Time frozen by the screenshot harness, or null to use the clock. */
  frozenTime: number | null;
}

const state: ViewState = {
  graph: { nodes: [], total: 0, scope: 'auto', relation: 'follow' },
  tokens: null,
  reducedMotion: false,
  selectedId: null,
  query: '',
  folded: new Set<string>(),
  frozenTime: null,
};

const railSpring = newSpring(0);
const railWidthSpring = newSpring(0);

let lastFrame = 0;
let rafHandle = 0;

/**
 * Freeze time for the screenshot harness.
 *
 * The harness needs the same frame every run, so the animation clock is driven
 * by a query parameter rather than by `performance.now()`. Without this a
 * screenshot taken mid-animation is different every time and cannot be
 * compared.
 */
function readFrozenTime(): number | null {
  const match = /[?&]testTime=([0-9.]+)/.exec(window.location.search);
  if (!match || match[1] === undefined) {
    return null;
  }
  const value = Number.parseFloat(match[1]);
  return Number.isFinite(value) ? Math.min(Math.max(value, 0), 1) : null;
}

function el<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) {
    throw new Error(`missing element: ${id}`);
  }
  return found as T;
}

function setVar(name: string, value: string): void {
  document.documentElement.style.setProperty(name, value);
}

function applyTokens(tokens: ThemeTokens): void {
  state.tokens = tokens;
  setVar('--tc-bg', tokens.background);
  setVar('--tc-panel', tokens.panel);
  setVar('--tc-raised', tokens.raised);
  setVar('--tc-border', tokens.border);
  setVar('--tc-text', tokens.text);
  setVar('--tc-muted', tokens.muted);
  setVar('--tc-bright', tokens.bright);
  setVar('--tc-accent', tokens.accent);
  setVar('--tc-selection', tokens.selection);
  setVar('--tc-selected-text', tokens.selectedText);
  setVar('--tc-error', tokens.error);
}

function post(message: GraphToHost): void {
  vscode.postMessage(message);
}

/** Announce a change for a screen reader, without narrating every frame. */
function announce(text: string): void {
  el('live').textContent = text;
}

/** Which nodes are shown, honouring folds and the search query. */
function visibleNodes(): GraphNodeDTO[] {
  const query = state.query.trim().toLowerCase();
  const nodes: GraphNodeDTO[] = [];
  let hideBelowDepth: number | null = null;
  for (const node of state.graph.nodes) {
    if (hideBelowDepth !== null) {
      if (node.depth > hideBelowDepth) {
        continue;
      }
      hideBelowDepth = null;
    }
    if (state.folded.has(node.id)) {
      hideBelowDepth = node.depth;
    }
    if (query.length > 0 && !matches(node, query)) {
      continue;
    }
    nodes.push(node);
  }
  return nodes;
}

function matches(node: GraphNodeDTO, query: string): boolean {
  return (
    node.title.toLowerCase().includes(query) ||
    node.subtitle.toLowerCase().includes(query) ||
    node.id.toLowerCase().includes(query)
  );
}

/** Render the rail tabs once; the indicator is the only thing that moves. */
function buildRail(): void {
  const tabs = el('rail-tabs');
  tabs.replaceChildren();
  for (const relation of RELATIONS) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'rail-tab';
    button.dataset['relation'] = relation;
    button.textContent = RELATION_LABELS[relation];
    button.setAttribute('role', 'tab');
    button.setAttribute('aria-selected', String(state.graph.relation === relation));
    button.addEventListener('click', () => post({ version: 1, type: 'changeRelation', relation }));
    tabs.appendChild(button);
  }
}

/** Move the indicator to the selected tab. */
function placeIndicator(instant: boolean): void {
  const tabs = el('rail-tabs');
  const selected = tabs.querySelector<HTMLElement>('[aria-selected="true"]');
  const indicator = el('rail-indicator');
  if (!selected) {
    indicator.style.opacity = '0';
    return;
  }
  indicator.style.opacity = '1';
  const target = selected.offsetLeft;
  const targetWidth = selected.offsetWidth;
  if (instant) {
    railSpring.value = target;
    railSpring.velocity = 0;
    railWidthSpring.value = targetWidth;
    railWidthSpring.velocity = 0;
  }
  indicator.style.transform = `translateX(${railSpring.value}px)`;
  indicator.style.width = `${railWidthSpring.value}px`;
}

function buildScopeMenu(): void {
  const menu = el('scope-menu');
  menu.replaceChildren();
  for (const scope of SCOPES) {
    const item = document.createElement('button');
    item.type = 'button';
    item.className = 'scope-item';
    item.setAttribute('role', 'option');
    item.setAttribute('aria-checked', String(state.graph.scope === scope));
    item.textContent = SCOPE_LABELS[scope];
    item.addEventListener('click', () => {
      closeScopeMenu();
      post({ version: 1, type: 'changeScope', scope });
    });
    menu.appendChild(item);
  }
  el('scope-label').textContent = SCOPE_LABELS[state.graph.scope];
}

function closeScopeMenu(): void {
  el('scope-menu').hidden = true;
  el('scope-trigger').setAttribute('aria-expanded', 'false');
}

function toggleScopeMenu(): void {
  const menu = el('scope-menu');
  const open = menu.hidden;
  menu.hidden = !open;
  el('scope-trigger').setAttribute('aria-expanded', String(open));
}

/** Draw the rows that fit the viewport. */
function renderRows(): void {
  const rows = el('rows');
  const viewport = el('viewport');
  const nodes = visibleNodes();
  const mode = widthModeFor(document.documentElement.clientWidth);
  const budget = mode === 'comfortable' ? 16 : mode === 'expanded' ? Number.POSITIVE_INFINITY : 0;

  rows.style.height = `${rowTop(nodes.length)}px`;
  rows.replaceChildren();

  const window = visibleWindow(viewport.scrollTop, viewport.clientHeight, nodes.length);
  const panelLeft = 0;
  const lineX = pathX(panelLeft);

  for (let index = window.start; index < window.end; index += 1) {
    const node = nodes[index];
    if (!node) {
      continue;
    }
    const row = document.createElement('div');
    row.className = 'row';
    row.style.top = `${index * graphGeometry.rowHeight}px`;

    // The vertical path line for this row's depth.
    const line = document.createElement('div');
    line.className = 'path-line';
    line.style.left = `${lineX - ROOT_PADDING}px`;
    line.style.top = '0';
    line.style.height = `${graphGeometry.rowHeight}px`;
    row.appendChild(line);

    // The card's own coordinate is measured from the panel edge, but it is
    // positioned inside the root's horizontal padding. Subtracting that padding
    // is what puts the first level at 42px on screen, which is the number the
    // drawing specifies and the geometry test asserts.
    const left = cardLeft(panelLeft, node.depth) - ROOT_PADDING;
    const right = graphGeometry.cardRightGap;
    const card = document.createElement('div');
    card.className = 'card';
    card.style.left = `${left}px`;
    card.style.right = `${right}px`;
    card.setAttribute('role', 'treeitem');
    card.setAttribute('tabindex', '0');
    card.setAttribute('aria-level', String(node.depth + 1));
    card.dataset['nodeId'] = node.id;
    if (node.id === state.selectedId) {
      card.dataset['selected'] = 'true';
    }

    const marker = document.createElement('span');
    marker.className = 'marker';
    marker.textContent = levelMarker(node.level);
    if (state.tokens) {
      marker.style.color = levelColor(state.tokens, node.level);
    }
    card.appendChild(marker);

    const labels = document.createElement('span');
    labels.className = 'labels';
    const title = document.createElement('div');
    title.className = 'card-title';
    title.textContent = node.title;
    labels.appendChild(title);

    // The subtitle is hidden in the narrowest mode, and the path is cut to the
    // budget so a long path cannot push the row into a scrollbar.
    if (mode !== 'extreme') {
      const subtitle = document.createElement('div');
      subtitle.className = 'card-subtitle';
      const text = node.relation
        ? `${relationLabel(node.relation)} · ${node.subtitle}`
        : node.subtitle;
      subtitle.textContent = cutPath(text, budget);
      labels.appendChild(subtitle);
    }
    card.appendChild(labels);

    // Only a node that really has children gets a fold control; a leaf with a
    // chevron is the bug the handoff calls out by name.
    if (node.hasChildren) {
      const fold = document.createElement('button');
      fold.type = 'button';
      fold.className = 'fold';
      fold.textContent = state.folded.has(node.id) ? '▸' : '▾';
      fold.setAttribute('aria-label', state.folded.has(node.id) ? '展开' : '折叠');
      fold.addEventListener('click', (event) => {
        event.stopPropagation();
        const folded = !state.folded.has(node.id);
        if (folded) {
          state.folded.add(node.id);
        } else {
          state.folded.delete(node.id);
        }
        post({ version: 1, type: 'fold', nodeId: node.id, folded });
        renderRows();
      });
      card.appendChild(fold);
    }

    card.addEventListener('click', () => {
      state.selectedId = node.id;
      renderRows();
      if (node.location) {
        post({
          version: 1,
          type: 'openLocation',
          uri: node.location.uri,
          line: node.location.line,
          column: node.location.column,
        });
      }
    });

    row.appendChild(card);
    rows.appendChild(row);
  }

  // Footer: what is shown, and why it may be less than everything.
  const hidden = state.graph.total - nodes.length;
  const shown = `${nodes.length} / ${state.graph.total}`;
  el('footer-left').textContent = hidden > 0 ? `显示 ${shown}，已折叠 ${hidden}` : `显示 ${shown}`;
  el('footer-right').textContent = SCOPE_LABELS[state.graph.scope];

  // Breadcrumb follows the selection, when there is one.
  const selected = nodes.find((node) => node.id === state.selectedId);
  el('breadcrumb').textContent = selected ? `${selected.title} · ${selected.subtitle}` : '';

  updateStates(nodes.length);
}

function cutPath(text: string, budget: number): string {
  if (!Number.isFinite(budget) || budget === 0) {
    return text;
  }
  return text.length > budget ? `${text.slice(0, budget)}…` : text;
}

/** Show exactly one of the state panels, or none. */
function updateStates(visibleCount: number): void {
  const loading = el('state-loading');
  const empty = el('state-empty');
  const noProvider = el('state-noprovider');
  const error = el('state-error');
  const degraded = state.graph.degraded;

  empty.hidden = true;
  noProvider.hidden = true;
  error.hidden = true;
  // While a request is running the spinner is the only thing shown: an empty
  // panel under it says the result is already in.
  if (!loading.hidden) {
    return;
  }
  if (degraded) {
    noProvider.hidden = false;
    return;
  }
  if (visibleCount === 0) {
    empty.hidden = false;
  }
}

/** Run the animation loop only while something is moving. */
function ensureFrame(): void {
  if (rafHandle !== 0) {
    return;
  }
  rafHandle = requestAnimationFrame(tick);
}

function tick(now: number): void {
  rafHandle = 0;
  const dt = state.frozenTime === null ? (lastFrame === 0 ? 1 / 60 : Math.min((now - lastFrame) / 1000, 0.1)) : 1 / 60;
  lastFrame = now;

  const tabs = el('rail-tabs');
  const selected = tabs.querySelector<HTMLElement>('[aria-selected="true"]');
  if (selected) {
    stepSpring(railSpring, selected.offsetLeft, dt, 'follow');
    stepSpring(railWidthSpring, selected.offsetWidth, dt, 'follow');
    const indicator = el('rail-indicator');
    indicator.style.transform = `translateX(${railSpring.value}px)`;
    indicator.style.width = `${railWidthSpring.value}px`;
    const settled =
      Math.abs(railSpring.value - selected.offsetLeft) < 0.5 &&
      Math.abs(railWidthSpring.value - selected.offsetWidth) < 0.5;
    if (!settled) {
      ensureFrame();
    }
  }
}

function handleMessage(message: HostToGraph): void {
  switch (message.type) {
    case 'ready':
      post({ version: 1, type: 'ready' });
      break;
    case 'setTheme':
      applyTokens(message.theme);
      renderRows();
      break;
    case 'setReducedMotion':
      state.reducedMotion = message.reduced;
      document.body.dataset['reducedMotion'] = String(message.reduced);
      break;
    case 'replaceGraph': {
      const snapshot = parseSnapshot(message.graph);
      if (!snapshot) {
        showError('关系数据格式不正确');
        return;
      }
      state.graph = snapshot;
      el('state-loading').hidden = true;
      buildRail();
      buildScopeMenu();
      placeIndicator(true);
      renderRows();
      announce(`已显示 ${snapshot.nodes.length} 个节点`);
      break;
    }
    case 'setState':
      if (message.state === 'loading') {
        // Showing the spinner also hides the other panels: an earlier render
        // may have left the empty panel up, and two panels at once claims the
        // result is both pending and empty.
        el('state-loading').hidden = false;
        el('state-empty').hidden = true;
        el('state-noprovider').hidden = true;
        el('state-error').hidden = true;
        announce('正在分析当前范围');
      } else if (message.state === 'error') {
        el('state-loading').hidden = true;
        showError(message.message ?? '无法读取关系');
      } else if (message.state === 'empty') {
        el('state-loading').hidden = true;
        state.graph = { nodes: [], total: 0, scope: state.graph.scope, relation: state.graph.relation };
        renderRows();
      } else {
        el('state-loading').hidden = true;
        renderRows();
      }
      break;
    case 'focusNode':
      state.selectedId = message.nodeId;
      renderRows();
      break;
    case 'packageProgress':
      // The packaging animation is rendered by its own module; this view only
      // records the phase so the footer can name it.
      el('footer-right').textContent = message.progress.phase;
      break;
  }
}

function showError(text: string): void {
  // An error is the only thing worth showing: leaving the empty panel up beside
  // it says the graph is both empty and broken.
  el('state-loading').hidden = true;
  el('state-empty').hidden = true;
  el('state-noprovider').hidden = true;
  el('error-message').textContent = text;
  el('state-error').hidden = false;
  announce(`错误：${text}`);
}

function wireControls(): void {
  el('refresh').addEventListener('click', () => post({ version: 1, type: 'refresh' }));
  el('search-toggle').addEventListener('click', () => {
    const row = el('search-row');
    row.hidden = !row.hidden;
    if (!row.hidden) {
      el<HTMLInputElement>('search-input').focus();
    }
  });
  el('scope-trigger').addEventListener('click', toggleScopeMenu);
  el('cancel').addEventListener('click', () => {
    el('state-loading').hidden = true;
    post({ version: 1, type: 'refresh' });
  });
  el('empty-action').addEventListener('click', () => post({ version: 1, type: 'refresh' }));
  el('error-retry').addEventListener('click', () => post({ version: 1, type: 'refresh' }));

  const input = el<HTMLInputElement>('search-input');
  let timer: number | undefined;
  input.addEventListener('input', () => {
    const clear = el('search-clear');
    clear.hidden = input.value.length === 0;
    if (timer !== undefined) {
      window.clearTimeout(timer);
    }
    // 150ms, the debounce the handoff specifies.
    timer = window.setTimeout(() => {
      state.query = input.value;
      renderRows();
      post({ version: 1, type: 'search', query: input.value });
    }, 150);
  });
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      if (input.value.length > 0) {
        input.value = '';
        state.query = '';
        el('search-clear').hidden = true;
        renderRows();
      } else {
        el('search-row').hidden = true;
        el('viewport').focus();
      }
    }
  });
  el('search-clear').addEventListener('click', () => {
    input.value = '';
    state.query = '';
    el('search-clear').hidden = true;
    renderRows();
  });

  el('viewport').addEventListener('scroll', () => renderRows());

  // Escape closes the scope list, and a click outside it does too.
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      closeScopeMenu();
    }
  });
  document.addEventListener('click', (event) => {
    const target = event.target;
    if (target instanceof Node && !el('scope-menu').contains(target) && !el('scope-trigger').contains(target)) {
      closeScopeMenu();
    }
  });

  window.addEventListener('resize', () => renderRows());
}

window.addEventListener('message', (event: MessageEvent<unknown>) => {
  const data = event.data;
  if (typeof data !== 'object' || data === null) {
    return;
  }
  handleMessage(data as HostToGraph);
});

function start(): void {
  state.frozenTime = readFrozenTime();
  wireControls();
  buildRail();
  buildScopeMenu();
  placeIndicator(true);
  renderRows();
  post({ version: 1, type: 'ready' });
  ensureFrame();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', start);
} else {
  start();
}

// The harness drives frames by hand, so the loop is exposed rather than being
// a private detail of the module.
export { REDUCED_MOTION_MS, parseNode };
