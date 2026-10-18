/**
 * Geometry assertions from the handoff's construction drawing.
 *
 * These are the tests that make "does it look right" answerable with numbers,
 * which matters because the implementer cannot see the result. Each assertion
 * names the tolerance the handoff allows.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  cardLeft,
  graphGeometry,
  layout,
  metrics,
  pathBudget,
  pathX,
  rowTop,
  space,
  visibleWindow,
  widthModeFor,
} from '../graph/geometry';

test('the vertical order of the view is the one in the drawing', () => {
  // 12 top margin, title 32, 8 gap, rail 32, 8 gap, search 32, 8 gap,
  // breadcrumb 24, 8 gap => viewport starts at 164.
  assert.equal(layout.titleTop, 12);
  assert.equal(layout.titleTop + metrics.titleRow + layout.gap, layout.railTop);
  assert.equal(layout.railTop + metrics.controlHeight + layout.gap, layout.searchTop);
  assert.equal(layout.searchTop + layout.searchRow + layout.gap, layout.breadcrumbTop);
  assert.equal(layout.breadcrumbTop + metrics.breadcrumb + layout.gap, layout.viewportTop);
});

test('the rows follow the handoff within its tolerance', () => {
  // Title row 32+/-1px, control 32+/-1px, breadcrumb 24+/-1px, footer 34+/-1px.
  assert.ok(Math.abs(metrics.titleRow - 32) <= 1, 'title row');
  assert.ok(Math.abs(metrics.controlHeight - 32) <= 1, 'control height');
  assert.ok(Math.abs(metrics.breadcrumb - 24) <= 1, 'breadcrumb');
  assert.ok(Math.abs(metrics.footer - 34) <= 1, 'footer');
  // Icon hit boxes are at least 28x28.
  assert.ok(metrics.iconHit >= 28);
  // Primary buttons are at least 32 high.
  assert.ok(metrics.primaryButton >= 32);
});

test('the graph rows are the same numbers the native core uses', () => {
  // These are read from src/graph_core.rs; if they change there, this fails.
  assert.equal(graphGeometry.rowHeight, 58);
  assert.equal(graphGeometry.firstRowTop, 96);
  assert.equal(graphGeometry.depthIndent, 16);
  assert.equal(graphGeometry.pathOffset, 30);
  assert.equal(graphGeometry.cardGap, 12);
  assert.equal(graphGeometry.markerInset, 6);
  assert.equal(graphGeometry.markerWidth, 12);
  assert.equal(graphGeometry.maxNodes, 240);
  // The card fits inside its slot with the two gaps the drawing specifies.
  assert.equal(graphGeometry.cardHeight + 2 * graphGeometry.cardGapY, graphGeometry.rowHeight);
});

test('the first level of cards sits 42px from the panel edge', () => {
  // The handoff states this exactly: path line at 30, gap 12, so 42.
  assert.equal(pathX(0), 30);
  assert.equal(cardLeft(0, 0), 42);
  // Within the stated 1px tolerance of the drawing.
  assert.ok(Math.abs(cardLeft(0, 0) - 42) <= 1);
  assert.ok(Math.abs(pathX(0) - 30) <= 1);
});

test('each level moves right by exactly the indent', () => {
  for (let depth = 0; depth < 8; depth += 1) {
    const inner = cardLeft(0, depth + 1);
    const outer = cardLeft(0, depth);
    assert.equal(inner - outer, graphGeometry.depthIndent, `depth ${depth}`);
    // Within the stated 1px tolerance.
    assert.ok(Math.abs(inner - outer - 16) <= 1);
  }
});

test('a panel offset moves the whole structure without changing spacing', () => {
  assert.equal(cardLeft(120, 0) - cardLeft(0, 0), 120);
  assert.equal(pathX(120) - pathX(0), 120);
  assert.equal(cardLeft(120, 2) - cardLeft(120, 1), graphGeometry.depthIndent);
});

test('a broken depth cannot produce a negative position', () => {
  // A node arriving with a nonsense depth must still draw inside the panel.
  assert.equal(cardLeft(0, -5), cardLeft(0, 0));
  assert.equal(cardLeft(0, Number.NaN), cardLeft(0, 0));
  assert.ok(cardLeft(0, 1e9) > 0);
  assert.equal(rowTop(-3), graphGeometry.firstRowTop);
  assert.equal(rowTop(Number.NaN), graphGeometry.firstRowTop);
});

test('row tops advance by the row height', () => {
  assert.equal(rowTop(0), 96);
  assert.equal(rowTop(1), 96 + 58);
  assert.equal(rowTop(10) - rowTop(9), 58);
});

test('the width mode is chosen from the view, not the window', () => {
  // The boundaries are the ones in the handoff table.
  assert.equal(widthModeFor(219), 'extreme');
  assert.equal(widthModeFor(220), 'compact');
  assert.equal(widthModeFor(309), 'compact');
  assert.equal(widthModeFor(310), 'comfortable');
  assert.equal(widthModeFor(429), 'comfortable');
  assert.equal(widthModeFor(430), 'expanded');
  // A view narrower than the narrowest mode is still a mode, not a crash.
  assert.equal(widthModeFor(0), 'extreme');
  assert.equal(widthModeFor(Number.NaN), 'extreme');
  assert.equal(widthModeFor(-100), 'extreme');
});

test('the path budget matches the mode table', () => {
  // Paths are hidden when narrow, cut at 16 characters when comfortable.
  assert.equal(pathBudget('extreme'), 0);
  assert.equal(pathBudget('compact'), 0);
  assert.equal(pathBudget('comfortable'), 16);
  assert.equal(pathBudget('expanded'), Number.POSITIVE_INFINITY);
});

test('only the visible rows and a small buffer are rendered', () => {
  // A 500px viewport holds about 8 rows, plus 3 either side.
  const window = visibleWindow(0, 500, 100);
  assert.equal(window.start, 0);
  assert.ok(window.end >= 8 + 3, `end was ${window.end}`);
  assert.ok(window.end <= 12, `end was ${window.end}`);

  // Scrolled down, the window follows and keeps its buffer.
  const scrolled = visibleWindow(58 * 20, 500, 100);
  assert.equal(scrolled.start, 20 - 3);
  assert.ok(scrolled.end > scrolled.start);

  // The window never runs past the end of the list.
  const atEnd = visibleWindow(58 * 98, 500, 100);
  assert.equal(atEnd.end, 100);

  // An empty graph and a zero viewport give an empty window rather than a
  // negative index.
  assert.deepEqual(visibleWindow(0, 500, 0), { start: 0, end: 0 });
  assert.deepEqual(visibleWindow(0, 0, 100), { start: 0, end: 0 });
  assert.deepEqual(visibleWindow(-50, -50, 100), { start: 0, end: 0 });
});

test('the spacing scale is the documented one', () => {
  assert.equal(space.s1, 4);
  assert.equal(space.s2, 8);
  assert.equal(space.s3, 12);
  assert.equal(space.s4, 16);
  assert.equal(space.s6, 24);
  assert.equal(space.s8, 32);
  // Root padding is 12 on both sides.
  assert.equal(metrics.rootPaddingX, 12);
  assert.equal(metrics.controlRadius, 8);
  assert.equal(metrics.semanticRail, 2);
});
