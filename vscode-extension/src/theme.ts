/**
 * Colour tokens for the graph webview.
 *
 * These values are the ones in `src/theme_core.rs`, so a card in the extension
 * is the same colour as a card in the native window. They are also what the
 * TC-IDE Dark and Light themes map onto, which is why the two are defined here
 * rather than being retyped in the CSS: a colour that appears in three places
 * will eventually disagree with itself.
 */

import type { NodeLevel, ThemeTokens } from './graph/protocol';

/** Blue-grey depth, from the approved whole-app preview. */
export const darkPalette: ThemeTokens = {
  background: '#0D1219',
  panel: '#151D27',
  raised: '#1D2937',
  border: '#2B394A',
  text: '#D7E2ED',
  muted: '#91A0B2',
  bright: '#EDF4FB',
  accent: '#78BAFF',
  selection: '#243D57',
  selectedText: '#EDF4FB',
  error: '#FF375F',
  level: {
    project: '#78BAFF',
    folder: '#9FD7FF',
    file: '#D7E2ED',
    class: '#30D158',
    function: '#BF5AF2',
    variable: '#E8E8ED',
    call: '#FF9F0A',
    definition: '#64D2FF',
  },
};

/** The light palette is its own set of values, not the dark one inverted. */
export const lightPalette: ThemeTokens = {
  background: '#F5F5F7',
  panel: '#FFFFFF',
  raised: '#F2F2F7',
  border: '#D1D1D6',
  text: '#3A3A3C',
  muted: '#6E6E73',
  bright: '#1D1D1F',
  accent: '#007AFF',
  selection: '#DBEAFE',
  selectedText: '#0A5AA8',
  error: '#9E1B3D',
  level: {
    project: '#007AFF',
    folder: '#1B5E8A',
    file: '#3A3A3C',
    class: '#146B2E',
    function: '#71317F',
    variable: '#24292F',
    call: '#8A4B08',
    definition: '#0B5394',
  },
};

/** The colour of a level's marker. */
export function levelColor(tokens: ThemeTokens, level: NodeLevel): string {
  return tokens.level[level] ?? tokens.muted;
}

/**
 * A short marker for a level, used where an icon font is not available.
 *
 * The handoff names codicons; a marker letter is the fallback so the strip is
 * still readable if the codicon font fails to load.
 */
export function levelMarker(level: NodeLevel): string {
  switch (level) {
    case 'project':
      return 'P';
    case 'folder':
      return 'D';
    case 'file':
      return 'F';
    case 'class':
      return 'C';
    case 'function':
      return 'f';
    case 'variable':
      return 'v';
    case 'call':
      return '→';
    case 'definition':
      return '=';
  }
}

/** The Chinese word for a relation, which the handoff fixes exactly. */
export function relationLabel(relation: 'definition' | 'call' | 'reference'): string {
  switch (relation) {
    case 'definition':
      return '定义';
    case 'call':
      return '调用';
    case 'reference':
      return '引用';
  }
}

/**
 * Turn tokens into the CSS custom properties the webview reads.
 *
 * VS Code's own variables come first in the CSS so the view follows the user's
 * theme; these are the fallbacks and the values the TC-IDE themes set.
 */
export function toCssVariables(tokens: ThemeTokens): Record<string, string> {
  return {
    '--tc-bg': tokens.background,
    '--tc-panel': tokens.panel,
    '--tc-raised': tokens.raised,
    '--tc-border': tokens.border,
    '--tc-text': tokens.text,
    '--tc-muted': tokens.muted,
    '--tc-bright': tokens.bright,
    '--tc-accent': tokens.accent,
    '--tc-selection': tokens.selection,
    '--tc-selected-text': tokens.selectedText,
    '--tc-error': tokens.error,
  };
}

/**
 * The card state matrix, as numbers rather than a description.
 *
 * `mix` is the share of the second colour, so a hovered card is the raised
 * colour with 10% of the text colour: a straight linear interpolation per
 * channel, which is what the handoff requires instead of an HSL guess.
 */
export interface CardVisual {
  fill: string;
  border: string;
  text: string;
  translateY: number;
  opacity: number;
  focusRing: boolean;
}

export type CardState = 'normal' | 'hover' | 'pressed' | 'selected' | 'disabled' | 'focus';

export function hexToRgb(hex: string): [number, number, number] {
  const cleaned = hex.replace('#', '');
  const value = Number.parseInt(cleaned.length === 3 ? cleaned.replace(/(.)/g, '$1$1') : cleaned, 16);
  if (!Number.isFinite(value)) {
    return [0, 0, 0];
  }
  return [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff];
}

export function rgbToHex(rgb: [number, number, number]): string {
  const clamped = rgb.map((channel) => Math.min(255, Math.max(0, Math.round(channel)))) as [number, number, number];
  return `#${clamped.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`.toUpperCase();
}

/** Linear interpolation of two colours per channel, rounded. */
export function mix(first: string, second: string, amount: number): string {
  const ratio = Number.isFinite(amount) ? Math.min(Math.max(amount, 0), 1) : 0;
  const a = hexToRgb(first);
  const b = hexToRgb(second);
  return rgbToHex([
    a[0] + (b[0] - a[0]) * ratio,
    a[1] + (b[1] - a[1]) * ratio,
    a[2] + (b[2] - a[2]) * ratio,
  ]);
}

/** The colours and offsets one card state draws with. */
export function cardVisual(tokens: ThemeTokens, state: CardState): CardVisual {
  switch (state) {
    case 'normal':
      return { fill: tokens.raised, border: tokens.border, text: tokens.text, translateY: 0, opacity: 1, focusRing: false };
    case 'hover':
      return {
        fill: mix(tokens.raised, tokens.text, 0.1),
        border: mix(tokens.border, tokens.accent, 0.45),
        text: tokens.bright,
        translateY: -1,
        opacity: 1,
        focusRing: false,
      };
    case 'pressed':
      return {
        fill: mix(tokens.raised, tokens.background, 0.35),
        border: mix(tokens.border, tokens.accent, 0.65),
        text: tokens.bright,
        translateY: 1,
        opacity: 1,
        focusRing: false,
      };
    case 'selected':
      return { fill: tokens.selection, border: tokens.accent, text: tokens.bright, translateY: 0, opacity: 1, focusRing: false };
    case 'disabled':
      return {
        fill: tokens.raised,
        border: mix(tokens.border, tokens.background, 0.5),
        text: tokens.muted,
        translateY: 0,
        opacity: 0.55,
        focusRing: false,
      };
    case 'focus':
      return { fill: tokens.raised, border: tokens.border, text: tokens.text, translateY: 0, opacity: 1, focusRing: true };
    default: {
      // Every CardState is handled above; this keeps the function total if a
      // new state is added without updating the switch.
      const exhaustive: never = state;
      void exhaustive;
      return { fill: tokens.raised, border: tokens.border, text: tokens.text, translateY: 0, opacity: 1, focusRing: false };
    }
  }
}

/**
 * Relative luminance and contrast, so a test can assert the handoff's 4.5:1.
 *
 * WCAG relative luminance, not a brightness average: the two disagree on
 * saturated colours, and the ratio is what the accessibility rule is about.
 */
function channelLuminance(channel: number): number {
  const value = channel / 255;
  return value <= 0.03928 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4);
}

export function relativeLuminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex);
  return 0.2126 * channelLuminance(r) + 0.7152 * channelLuminance(g) + 0.0722 * channelLuminance(b);
}

export function contrastRatio(first: string, second: string): number {
  const a = relativeLuminance(first);
  const b = relativeLuminance(second);
  const lighter = Math.max(a, b);
  const darker = Math.min(a, b);
  return (lighter + 0.05) / (darker + 0.05);
}
