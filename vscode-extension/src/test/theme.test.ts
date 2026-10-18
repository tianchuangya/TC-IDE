/**
 * Colour rules from the handoff: the palettes match the native core, the card
 * state matrix mixes per channel, and text meets the contrast floor.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  cardVisual,
  contrastRatio,
  darkPalette,
  hexToRgb,
  lightPalette,
  levelColor,
  levelMarker,
  mix,
  relationLabel,
  rgbToHex,
  toCssVariables,
} from '../theme';

test('the palettes are the ones in the native core', () => {
  // Read from src/theme_core.rs; a change there should change this test.
  assert.equal(darkPalette.background, '#0D1219');
  assert.equal(darkPalette.panel, '#151D27');
  assert.equal(darkPalette.raised, '#1D2937');
  assert.equal(darkPalette.border, '#2B394A');
  assert.equal(darkPalette.accent, '#78BAFF');
  assert.equal(lightPalette.background, '#F5F5F7');
  assert.equal(lightPalette.panel, '#FFFFFF');
  assert.equal(lightPalette.accent, '#007AFF');
  // The light palette is its own set of values, not the dark one inverted.
  assert.notEqual(lightPalette.background, darkPalette.background);
  assert.notEqual(lightPalette.accent, darkPalette.accent);
});

test('colour mixing is a per channel linear interpolation', () => {
  // The handoff requires this rather than an HSL guess, so the arithmetic is
  // pinned: black and white half way is 128 on every channel.
  assert.equal(mix('#000000', '#FFFFFF', 0.5), '#808080');
  assert.equal(mix('#000000', '#FFFFFF', 0), '#000000');
  assert.equal(mix('#000000', '#FFFFFF', 1), '#FFFFFF');
  // The square brackets are values from the handoff's own table.
  const hoveredBorder = mix('#2B394A', '#78BAFF', 0.45);
  const [r, g, b] = hexToRgb(hoveredBorder);
  assert.ok(r > 0x2b && r < 0x78, 'red moved towards the accent');
  assert.ok(g > 0x39 && g < 0xba, 'green moved towards the accent');
  assert.ok(b > 0x4a, 'blue moved towards the accent');
  // Out of range amounts are clamped rather than extrapolated.
  assert.equal(mix('#000000', '#FFFFFF', -1), '#000000');
  assert.equal(mix('#000000', '#FFFFFF', 2), '#FFFFFF');
});

test('a hex round trip keeps its channels', () => {
  for (const hex of ['#0D1219', '#78BAFF', '#FFFFFF', '#000000', '#2B394A']) {
    assert.equal(rgbToHex(hexToRgb(hex)), hex);
  }
  // Shorthand expands, and a malformed value does not throw.
  assert.equal(rgbToHex(hexToRgb('#fff')), '#FFFFFF');
  assert.deepEqual(hexToRgb('nonsense'), [0, 0, 0]);
});

test('the card states differ in the way the matrix says', () => {
  const normal = cardVisual(darkPalette, 'normal');
  const hover = cardVisual(darkPalette, 'hover');
  const pressed = cardVisual(darkPalette, 'pressed');
  const selected = cardVisual(darkPalette, 'selected');
  const disabled = cardVisual(darkPalette, 'disabled');
  const focus = cardVisual(darkPalette, 'focus');

  // Hover lifts by one, press sinks by one, selection does not move.
  assert.equal(normal.translateY, 0);
  assert.equal(hover.translateY, -1);
  assert.equal(pressed.translateY, 1);
  assert.equal(selected.translateY, 0);
  assert.equal(focus.translateY, 0);

  // Selection uses the selection colour and the accent border.
  assert.equal(selected.fill, darkPalette.selection);
  assert.equal(selected.border, darkPalette.accent);

  // Disabled is the only faded state, and the only one using muted text.
  assert.equal(disabled.opacity, 0.55);
  assert.equal(disabled.text, darkPalette.muted);
  assert.equal(normal.opacity, 1);

  // Focus adds a ring without changing the fill.
  assert.equal(focus.focusRing, true);
  assert.equal(focus.fill, normal.fill);
  assert.equal(normal.focusRing, false);

  // Pressed is darker than hovered, not a jump to black.
  const pressedLum = hexToRgb(pressed.fill).reduce((a, b) => a + b, 0);
  const hoverLum = hexToRgb(hover.fill).reduce((a, b) => a + b, 0);
  assert.ok(pressedLum < hoverLum, 'pressed is darker than hovered');
  assert.notEqual(pressed.fill, '#000000', 'pressed is not pure black');
});

test('text meets the contrast floor in both themes', () => {
  // The handoff requires at least 4.5:1 for text on its background.
  for (const [name, tokens] of [
    ['dark', darkPalette],
    ['light', lightPalette],
  ] as const) {
    const pairs: [string, string, string][] = [
      ['text on background', tokens.text, tokens.background],
      ['text on raised', tokens.text, tokens.raised],
      ['bright on panel', tokens.bright, tokens.panel],
      ['selected text on selection', tokens.selectedText, tokens.selection],
    ];
    for (const [label, foreground, background] of pairs) {
      const ratio = contrastRatio(foreground, background);
      assert.ok(ratio >= 4.5, `${name}: ${label} is ${ratio.toFixed(2)}:1`);
    }
    // The muted colour is for secondary text; it still has to be readable.
    const muted = contrastRatio(tokens.muted, tokens.background);
    assert.ok(muted >= 3, `${name}: muted on background is ${muted.toFixed(2)}:1`);
  }
});

test('the contrast helper agrees with the reference values', () => {
  // Black on white is the maximum, and a colour against itself is the minimum.
  assert.ok(Math.abs(contrastRatio('#000000', '#FFFFFF') - 21) < 0.1);
  assert.ok(Math.abs(contrastRatio('#FFFFFF', '#FFFFFF') - 1) < 0.001);
  // Order does not matter.
  assert.equal(
    contrastRatio('#0D1219', '#D7E2ED').toFixed(4),
    contrastRatio('#D7E2ED', '#0D1219').toFixed(4),
  );
});

test('every level has a colour and a marker', () => {
  const levels = ['project', 'folder', 'file', 'class', 'function', 'variable', 'call', 'definition'] as const;
  const markers = new Set<string>();
  for (const level of levels) {
    assert.match(levelColor(darkPalette, level), /^#[0-9A-F]{6}$/);
    assert.match(levelColor(lightPalette, level), /^#[0-9A-F]{6}$/);
    const marker = levelMarker(level);
    assert.ok(marker.length > 0, level);
    markers.add(marker);
  }
  // Every level is distinguishable at a glance.
  assert.equal(markers.size, levels.length);
});

test('the relation words are the ones the handoff fixes', () => {
  // A definition must never be shown as a call: that is a factual error, not a
  // wording choice.
  assert.equal(relationLabel('definition'), '定义');
  assert.equal(relationLabel('call'), '调用');
  assert.equal(relationLabel('reference'), '引用');
});

test('the css variables carry every colour the stylesheet reads', () => {
  const variables = toCssVariables(darkPalette);
  for (const key of [
    '--tc-bg',
    '--tc-panel',
    '--tc-raised',
    '--tc-border',
    '--tc-text',
    '--tc-muted',
    '--tc-bright',
    '--tc-accent',
    '--tc-selection',
    '--tc-selected-text',
    '--tc-error',
  ]) {
    assert.ok(key in variables, key);
    assert.match(variables[key] ?? '', /^#[0-9A-F]{6}$/, key);
  }
});
