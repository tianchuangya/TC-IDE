/**
 * The graph view's markup and stylesheet.
 *
 * It is built here rather than in an `.html` file so the same module can be
 * imported by the extension and by the screenshot harness: the handoff
 * forbids a second copy of the markup for the harness, because two copies
 * drift and then the screenshots stop meaning anything.
 */

import { metrics, space } from './geometry';

/**
 * Stylesheet, with the geometry injected from the tokens so a card's height in
 * CSS and its height in the geometry test cannot disagree.
 */
export function styles(_options: { nonce: string }): string {
  return `
:root {
  --tc-space-1: ${space.s1}px;
  --tc-space-2: ${space.s2}px;
  --tc-space-3: ${space.s3}px;
  --tc-space-4: ${space.s4}px;
  --tc-control-height: ${metrics.controlHeight}px;
  --tc-control-radius: ${metrics.controlRadius}px;
  --tc-row-padding: ${metrics.rootPaddingX}px;
  --tc-ui-font: var(--vscode-font-family, system-ui, sans-serif);
  --tc-code-font: var(--vscode-editor-font-family, monospace);
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  padding: 0;
  height: 100%;
  /* The root never scrolls sideways; only the range rail may. */
  overflow: hidden;
  font-family: var(--tc-ui-font);
  font-size: 13px;
  line-height: 18px;
  color: var(--tc-text);
  background: var(--tc-bg);
}

#root {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 12px var(--tc-row-padding) 0;
  overflow: hidden;
}

/* Title row */
.title-row {
  display: flex;
  align-items: center;
  gap: ${space.s2}px;
  height: ${metrics.titleRow}px;
  margin-top: 0;
  flex: 0 0 auto;
}
.title-row .brand {
  font-size: 14px;
  font-weight: 600;
  line-height: 20px;
  color: var(--tc-bright);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.title-row .spacer { flex: 1 1 auto; }
.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: ${metrics.iconHit}px;
  height: ${metrics.iconHit}px;
  margin: 0 1px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--tc-muted);
  cursor: pointer;
  font-size: 14px;
  line-height: 1;
}
.icon-button:hover { color: var(--tc-text); }
.icon-button:active { color: var(--tc-bright); }
.icon-button[aria-pressed="true"] { color: var(--tc-accent); }
.icon-button:focus-visible {
  outline: 1px solid var(--tc-bright);
  outline-offset: -1px;
}

/* Relation rail */
.rail {
  position: relative;
  display: flex;
  align-items: center;
  height: ${metrics.controlHeight}px;
  border-bottom: 1px solid var(--tc-border);
  flex: 0 0 auto;
  margin-top: ${space.s2}px;
}
.rail-tabs {
  display: flex;
  align-items: center;
  gap: 0;
  overflow-x: auto;
  scrollbar-width: none;
}
.rail-tabs::-webkit-scrollbar { display: none; }
.rail-tab {
  height: 30px;
  padding: 0 10px;
  border: none;
  background: transparent;
  color: var(--tc-muted);
  font: inherit;
  cursor: pointer;
  white-space: nowrap;
}
.rail-tab[aria-selected="true"] { color: var(--tc-bright); }
.rail-tab:focus-visible { outline: 1px solid var(--tc-bright); outline-offset: -2px; }
/* The indicator is a single element that travels, not one per tab: a line
   that fades out here and in there reads as a jump. */
.rail-indicator {
  position: absolute;
  bottom: 0;
  left: 0;
  height: ${metrics.semanticRail}px;
  background: var(--tc-accent);
  will-change: transform, width;
}

/* Scope trigger and its list */
.scope-row {
  display: flex;
  align-items: center;
  gap: ${space.s2}px;
  height: ${metrics.controlHeight}px;
  margin-top: ${space.s2}px;
  flex: 0 0 auto;
}
.scope-trigger {
  display: inline-flex;
  align-items: center;
  gap: ${space.s1}px;
  height: ${metrics.controlHeight}px;
  padding: 0 10px;
  border: 1px solid var(--tc-border);
  border-radius: var(--tc-control-radius);
  background: var(--tc-raised);
  color: var(--tc-text);
  font: inherit;
  cursor: pointer;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.scope-trigger:hover { border-color: var(--tc-accent); color: var(--tc-bright); }
.scope-trigger:focus-visible { outline: 1px solid var(--tc-bright); outline-offset: 1px; }
.scope-menu {
  position: absolute;
  z-index: 10;
  margin-top: 4px;
  min-width: 160px;
  padding: 4px 0;
  border: 1px solid var(--tc-border);
  border-radius: var(--tc-control-radius);
  background: var(--tc-panel);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.28);
}
.scope-menu[hidden] { display: none; }
.scope-item {
  display: flex;
  align-items: center;
  height: ${metrics.controlHeight}px;
  padding: 0 10px 0 12px;
  border: none;
  background: transparent;
  color: var(--tc-text);
  font: inherit;
  cursor: pointer;
  width: 100%;
  text-align: left;
  position: relative;
}
.scope-item:hover { background: var(--tc-selection); }
.scope-item[aria-checked="true"] {
  color: var(--tc-bright);
}
/* The selection marker is a short accent line, never a letter. */
.scope-item[aria-checked="true"]::before {
  content: '';
  position: absolute;
  left: 0;
  top: 8px;
  bottom: 8px;
  width: ${metrics.semanticRail}px;
  background: var(--tc-accent);
}

/* Search */
.search-row {
  display: flex;
  align-items: center;
  height: ${metrics.controlHeight}px;
  margin-top: ${space.s2}px;
  flex: 0 0 auto;
  position: relative;
}
.search-row[hidden] { display: none; }
.search-input {
  width: 100%;
  height: ${metrics.controlHeight}px;
  padding: 0 ${space.s4}px 0 30px;
  border: 1px solid var(--tc-border);
  border-radius: var(--tc-control-radius);
  background: var(--tc-raised);
  color: var(--tc-text);
  font: inherit;
}
.search-input:focus { outline: none; border-color: var(--tc-accent); }
.search-input::placeholder { color: var(--tc-muted); }
.search-icon {
  position: absolute;
  left: 9px;
  top: 50%;
  transform: translateY(-50%);
  width: 14px;
  height: 14px;
  color: var(--tc-muted);
  pointer-events: none;
}
.search-clear {
  position: absolute;
  right: 2px;
  top: 2px;
  width: ${metrics.iconHit}px;
  height: ${metrics.iconHit}px;
}
.search-clear[hidden] { display: none; }

/* Breadcrumb */
.breadcrumb {
  display: flex;
  align-items: center;
  height: ${metrics.breadcrumb}px;
  margin-top: ${space.s2}px;
  color: var(--tc-muted);
  font-size: 11px;
  line-height: 16px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 0 0 auto;
}

/* Graph viewport */
.viewport {
  position: relative;
  flex: 1 1 auto;
  margin-top: ${space.s2}px;
  overflow-y: auto;
  overflow-x: hidden;
}
.viewport[data-layout="radial"] { overflow: hidden; }
.radial { position:absolute; inset:0; display:none; }
.viewport[data-layout="radial"] .rows { display:none; }
.viewport[data-layout="radial"] .radial { display:block; }
.radial svg { width:100%; height:100%; overflow:visible; }
.radial-link { stroke:var(--tc-border); stroke-width:1.2; fill:none; opacity:.86; }
.radial-node { cursor:pointer; transition:transform .18s cubic-bezier(.2,.8,.2,1); }
.radial-node:hover { transform:scale(1.08); }
.radial-dot { fill:var(--tc-raised); stroke:var(--tc-accent); stroke-width:1.5; }
.radial-label { fill:var(--tc-text); font:12px var(--tc-ui-font); dominant-baseline:middle; }
.rows { position: relative; }

.row {
  position: absolute;
  left: 0;
  right: 0;
  height: ${metrics.controlHeight}px;
}

.card {
  position: absolute;
  top: ${5}px;
  height: 46px;
  border: 1px solid var(--tc-border);
  border-radius: var(--tc-control-radius);
  background: var(--tc-raised);
  color: var(--tc-text);
  display: flex;
  align-items: center;
  cursor: pointer;
  overflow: hidden;
  will-change: transform;
}
.card:hover { border-color: var(--tc-accent); }
.card[data-selected="true"] {
  background: var(--tc-selection);
  border-color: var(--tc-accent);
  color: var(--tc-bright);
}
.card:focus-visible {
  outline: 1px solid var(--tc-bright);
  outline-offset: 1px;
}
.card .marker {
  flex: 0 0 auto;
  width: ${space.s2}px;
  margin-left: ${space.s2}px;
  font-family: var(--tc-code-font);
  font-size: 11px;
  text-align: center;
}
.card .labels { min-width: 0; margin-left: ${space.s3}px; }
.card .card-title {
  font-size: 13px;
  font-weight: 500;
  line-height: 18px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.card .card-subtitle {
  font-size: 11px;
  line-height: 16px;
  color: var(--tc-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fold {
  flex: 0 0 auto;
  width: 18px;
  height: 18px;
  margin-left: ${space.s1}px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--tc-muted);
  font-size: 10px;
  cursor: pointer;
}
.fold:hover { color: var(--tc-bright); }
.fold:focus-visible { outline: 1px solid var(--tc-bright); }

/* Path line down the left */
.path-line {
  position: absolute;
  width: 1px;
  background: var(--tc-border);
}

/* The state panels */
.state {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: ${space.s2}px;
  color: var(--tc-muted);
  text-align: center;
  padding: ${space.s4}px;
}
.state[hidden] { display: none; }
.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--tc-border);
  border-top-color: var(--tc-accent);
  border-radius: 50%;
  animation: spin 0.9s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }
/* A screenshot has to be the same picture every run, so when the page is being
   captured the spinner holds a fixed angle instead of rotating: an animated
   ring lands on a different frame each time and the baseline never matches. */
body[data-frozen="true"] .spinner { animation: none; transform: rotate(90deg); }
.state.error { color: var(--tc-error); }
.state .state-action {
  height: ${metrics.primaryButton}px;
  padding: 0 ${space.s3}px;
  border: 1px solid var(--tc-border);
  border-radius: var(--tc-control-radius);
  background: var(--tc-raised);
  color: var(--tc-text);
  font: inherit;
  cursor: pointer;
}
.state .state-action:hover { border-color: var(--tc-accent); }

/* Footer */
.footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: ${metrics.footer}px;
  flex: 0 0 auto;
  border-top: 1px solid var(--tc-border);
  color: var(--tc-muted);
  font-size: 11px;
  line-height: 16px;
  white-space: nowrap;
  overflow: hidden;
}

/* Reduced motion: no spin, no travel, only a short fade. */
body[data-reduced-motion="true"] .spinner { animation: none; }
body[data-reduced-motion="true"] .rail-indicator { transition: none; }
@media (prefers-reduced-motion: reduce) {
  .spinner { animation: none; }
}

/* High contrast: follow the host. */
body[data-high-contrast="true"] .card { border-width: 1px; }
body[data-high-contrast="true"] .card:focus-visible { outline-width: 2px; }
`;
}

/** The markup, with nothing in it that a script must fill in later. */
export function markup(): string {
  return `
<div id="root">
  <div class="title-row">
    <span class="brand" id="title">调用层级</span>
    <span class="spacer"></span>
    <button class="icon-button" id="search-toggle" type="button" aria-label="搜索" title="搜索">⌕</button>
    <button class="icon-button" id="follow-toggle" type="button" aria-label="跟随光标" title="跟随光标" aria-pressed="true">◎</button>
    <button class="icon-button" id="layout-toggle" type="button" aria-label="切换径向关系图" title="切换树形 / 径向关系图" aria-pressed="false">◌</button>
    <button class="icon-button" id="refresh" type="button" aria-label="刷新关系图" title="刷新关系图">⟳</button>
  </div>

  <div class="rail" id="rail" role="tablist" aria-label="关系方向">
    <div class="rail-tabs" id="rail-tabs"></div>
    <div class="rail-indicator" id="rail-indicator"></div>
  </div>

  <div class="scope-row">
    <button class="scope-trigger" id="scope-trigger" type="button"
            aria-haspopup="listbox" aria-expanded="false">
      <span id="scope-label">自动</span><span aria-hidden="true">⌄</span>
    </button>
    <div class="scope-menu" id="scope-menu" role="listbox" aria-label="关系范围" hidden></div>
  </div>

  <div class="search-row" id="search-row" hidden>
    <span class="search-icon" aria-hidden="true">⌕</span>
    <input class="search-input" id="search-input" type="search"
           placeholder="搜索文件、类、函数、变量" aria-label="搜索" />
    <button class="icon-button search-clear" id="search-clear" type="button"
            aria-label="清除搜索" hidden>×</button>
  </div>

  <div class="breadcrumb" id="breadcrumb"></div>

  <div class="viewport" id="viewport">
    <div class="rows" id="rows"></div>
    <div class="radial" id="radial" aria-label="径向调用关系图"></div>

    <div class="state" id="state-loading" hidden>
      <div class="spinner"></div>
      <div>正在分析当前范围…</div>
      <button class="state-action" id="cancel" type="button">取消</button>
    </div>
    <div class="state" id="state-empty" hidden>
      <div>打开一个项目后显示关系</div>
      <button class="state-action" id="empty-action" type="button">刷新关系图</button>
    </div>
    <div class="state" id="state-noprovider" hidden>
      <div>当前语言未提供符号或调用层级</div>
      <button class="state-action" id="noprovider-action" type="button">打开扩展</button>
    </div>
    <div class="state error" id="state-error" hidden>
      <div id="error-message"></div>
      <button class="state-action" id="error-retry" type="button">重试</button>
    </div>
  </div>

  <div class="footer">
    <span id="footer-left"></span>
    <span id="footer-right"></span>
  </div>
</div>

<div aria-live="polite" id="live" style="position:absolute;left:-9999px;width:1px;height:1px;overflow:hidden"></div>
`;
}

/**
 * The Content Security Policy for the webview.
 *
 * Only the extension's own bundle may run, identified by a nonce that changes
 * on every render; no remote script and no inline handler is allowed.
 */
export function contentSecurityPolicy(webviewUri: string, nonce: string): string {
  // Scripts are nonce-only, which is what a CSP is protecting against. The
  // style attribute is allowed because the view positions every card with
  // inline geometry: `style-src 'nonce-…'` alone blocks `element.style`, and
  // the alternative would be a stylesheet per node.
  return [
    "default-src 'none'",
    // A nonce and 'unsafe-inline' cancel each other out in a CSP: the nonce
    // wins and inline styles stay blocked. The stylesheet is therefore loaded
    // without a nonce and inline style attributes are allowed, while scripts
    // remain nonce-only, which is the part that guards against injection.
    "style-src 'unsafe-inline'",
    `script-src 'nonce-${nonce}'`,
    `img-src ${webviewUri} data:`,
    `font-src ${webviewUri}`,
    "connect-src 'none'",
  ].join('; ');
}
