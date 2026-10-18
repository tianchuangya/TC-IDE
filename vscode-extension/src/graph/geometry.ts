/**
 * Tokens from the handoff, kept in one place so the geometry tests and the
 * drawing read the same numbers.
 *
 * Every value here is a CSS logical pixel. Browser zoom and system DPI are
 * left to VS Code and Chromium; multiplying by devicePixelRatio by hand would
 * double scale on a HiDPI screen.
 *
 * The graph constants are the same values the native backend uses
 * (`src/graph_core.rs`), so the two products cannot drift apart in spacing.
 */

export const space = {
  s1: 4,
  s2: 8,
  s3: 12,
  s4: 16,
  s6: 24,
  s8: 32,
} as const;

export const metrics = {
  /** Root left and right padding of the view. */
  rootPaddingX: 12,
  /** Title row height. */
  titleRow: 32,
  /** Height of the relation rail and of the scope trigger row. */
  controlHeight: 32,
  /** Border radius of cards, inputs and buttons. */
  controlRadius: 8,
  /** Breadcrumb row height. */
  breadcrumb: 24,
  /** Footer height. */
  footer: 34,
  /** Thickness of an active tab, range and relation focus line. */
  semanticRail: 2,
  /** Icon hit box. */
  iconHit: 28,
  /** Primary button minimum height. */
  primaryButton: 32,
} as const;

/**
 * Vertical order of the view, from the handoff's construction drawing.
 *
 * The search row collapses to zero height, but its 8px gap is kept so the
 * breadcrumb does not jump up against the rail.
 */
export const layout = {
  titleTop: 12,
  railTop: 52,
  searchTop: 92,
  breadcrumbTop: 132,
  viewportTop: 164,
  /** Gap between rows. */
  gap: 8,
  /** Search row height, removed when collapsed. */
  searchRow: 32,
} as const;

/**
 * Graph row geometry, taken from the native shared core.
 *
 * `cardHeight` is not in the handoff as a number; it is the row height minus
 * the two 6px gaps the drawing specifies above and below the card.
 */
export const graphGeometry = {
  rowHeight: 58,
  firstRowTop: 96,
  depthIndent: 16,
  pathOffset: 30,
  cardGap: 12,
  markerInset: 6,
  markerWidth: 12,
  cardHeight: 46,
  cardGapY: 6,
  cardRightGap: 8,
  titleTop: 7,
  subtitleBaseline: 18,
  iconSize: 14,
  iconLeft: 8,
  titleGap: 7,
  maxNodes: 240,
} as const;

/** Width at which each layout mode takes over. */
export const widthModes = {
  /** Below this the view shows only titles. */
  extreme: 220,
  compact: 310,
  comfortable: 430,
} as const;

export type WidthMode = 'extreme' | 'compact' | 'comfortable' | 'expanded';

/**
 * Pick a layout mode from the view's own width.
 *
 * The view is measured, never the window: the same view is usable pinned left
 * or moved to the secondary side bar, and a window wide enough for the
 * expanded mode with a 200px view must still choose the narrow one.
 */
export function widthModeFor(clientWidth: number): WidthMode {
  if (!Number.isFinite(clientWidth) || clientWidth < widthModes.extreme) {
    return 'extreme';
  }
  if (clientWidth < widthModes.compact) {
    return 'compact';
  }
  if (clientWidth < widthModes.comfortable) {
    return 'comfortable';
  }
  return 'expanded';
}

/** Characters of a path shown before an ellipsis, per mode. */
export function pathBudget(mode: WidthMode): number {
  switch (mode) {
    case 'extreme':
      return 0;
    case 'compact':
      return 0;
    case 'comfortable':
      return 16;
    case 'expanded':
      return Number.POSITIVE_INFINITY;
  }
}

/**
 * Left edge of a card at a depth.
 *
 * The first level sits at the path line plus its gap; each further level moves
 * right by the indent, which is what makes the hierarchy readable from
 * indentation alone.
 */
export function cardLeft(panelLeft: number, depth: number): number {
  const safeDepth = Number.isFinite(depth) ? Math.max(0, Math.floor(depth)) : 0;
  return panelLeft + graphGeometry.pathOffset + graphGeometry.cardGap + safeDepth * graphGeometry.depthIndent;
}

/** Left edge of the vertical path line. */
export function pathX(panelLeft: number): number {
  return panelLeft + graphGeometry.pathOffset;
}

/** Vertical top of the row at an index, measured from the viewport top. */
export function rowTop(index: number): number {
  const safe = Number.isFinite(index) ? Math.max(0, Math.floor(index)) : 0;
  return graphGeometry.firstRowTop + safe * graphGeometry.rowHeight;
}

/**
 * How many rows fit a viewport height, and which slice to render.
 *
 * Only the visible rows and a small buffer above and below are drawn; a
 * project with more nodes than fit reports a larger total rather than
 * rendering everything.
 */
export function visibleWindow(
  scrollTop: number,
  viewportHeight: number,
  totalRows: number,
  buffer = 3,
): { start: number; end: number } {
  const rowHeight = graphGeometry.rowHeight;
  const safeScroll = Number.isFinite(scrollTop) ? Math.max(0, scrollTop) : 0;
  const safeHeight = Number.isFinite(viewportHeight) ? Math.max(0, viewportHeight) : 0;
  const safeTotal = Number.isFinite(totalRows) ? Math.max(0, Math.floor(totalRows)) : 0;
  if (safeTotal === 0 || safeHeight === 0) {
    return { start: 0, end: 0 };
  }
  const first = Math.floor(safeScroll / rowHeight) - buffer;
  const last = Math.ceil((safeScroll + safeHeight) / rowHeight) + buffer;
  const start = Math.max(0, first);
  const end = Math.min(safeTotal, Math.max(start, last));
  return { start, end };
}
