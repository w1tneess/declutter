// WCAG 2.2 AA over the design tokens, in both themes: text 4.5:1, control borders, focus ring and graphics 3:1.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// vitest runs from ui/.
const css = readFileSync('src/design/tokens.css', 'utf8');


function block(selector: string): Record<string, string> {
  const start = css.indexOf(selector);
  if (start < 0) throw new Error(`missing ${selector}`);
  const body = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
  return Object.fromEntries([...body.matchAll(/--([\w-]+):\s*([^;]+);/g)].map((m) => [m[1] as string, (m[2] as string).trim()]));
}

const light = block(':root {');
const dark = { ...light, ...block(":root[data-theme='dark'] {") };

function luminance(hex: string): number {
  const m = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!m) throw new Error(`not a #rrggbb colour: ${hex}`);
  const [r, g, b] = [0, 2, 4].map((i) => {
    const c = parseInt((m[1] as string).slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(a: string, b: string): number {
  const [x, y] = [luminance(a), luminance(b)].sort((p, q) => q - p) as [number, number];
  return (x + 0.05) / (y + 0.05);
}

const BACKGROUNDS = ['bg', 'surface', 'surface-alt', 'surface-hover', 'selected'];
// [foreground, backgrounds, minimum]
const PAIRS: [string, string[], number][] = [
  ['text', BACKGROUNDS, 4.5],
  ['text-muted', BACKGROUNDS, 4.5],
  ['accent-text', [...BACKGROUNDS, 'accent-soft'], 4.5],
  ['danger-text', ['surface', 'bg'], 4.5],
  ['warning-text', ['surface', 'surface-alt'], 4.5],
  ['success-text', ['surface', 'bg'], 4.5],
  ['on-accent', ['accent', 'accent-hover'], 4.5],
  ['on-danger', ['danger', 'danger-hover'], 4.5],
  ['border-control', ['bg', 'surface', 'surface-alt', 'surface-hover'], 3],
  ['focus', ['bg', 'surface', 'surface-alt', 'selected', 'accent-soft'], 3],
  ['accent', ['bg', 'surface', 'track'], 3],
];

describe.each([
  ['light', light],
  ['dark', dark],
])('%s theme', (_name, t) => {
  for (const [fg, bgs, min] of PAIRS)
    for (const bg of bgs)
      it(`${fg} on ${bg} is at least ${min}:1`, () => {
        expect(ratio(t[fg] as string, t[bg] as string)).toBeGreaterThanOrEqual(min);
      });
});

describe('tokens', () => {
  it('dark background is #202020, not black', () => expect(dark.bg).toBe('#202020'));

  it('the system-dark block and the forced-dark block are identical', () => {
    const media = block(":root:not([data-theme='light']) {");
    expect(media).toEqual(block(":root[data-theme='dark'] {"));
  });

  it('type scale is 12 / 13 / 14 / 20 / 32 and spacing sits on the 4 px grid', () => {
    const sizes = Object.entries(light).filter(([k]) => k.startsWith('fs-')).map(([, v]) => parseInt(v, 10));
    expect(sizes.sort((a, b) => a - b)).toEqual([12, 13, 14, 20, 32]);
    for (const [k, v] of Object.entries(light)) if (k.startsWith('sp-')) expect(parseInt(v, 10) % 4).toBe(0);
  });

  it('font is Segoe UI Variable', () => expect(light.font).toMatch(/^'Segoe UI Variable/));
});

it('ratio matches known values', () => {
  expect(ratio('#000000', '#ffffff')).toBeCloseTo(21, 5);
  expect(ratio('#767676', '#ffffff')).toBeCloseTo(4.54, 2);
});
