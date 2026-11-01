import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { defaultTheme, resolveTheme, themes } from './themes';

const css = readFileSync('src/themes/themes.css', 'utf8');
const shellCss = readFileSync('src/shell/styles.css', 'utf8');
const palettes: Record<string, Record<string, string>> = Object.fromEntries(
  [...css.matchAll(/:root\[data-theme='([a-z]+)'\]\s*\{([^}]+)\}/g)].map(
    ([, id = '', block = '']) => [
      id,
      Object.fromEntries(
        [...block.matchAll(/--([\w-]+):\s*(#[\da-f]{6})/g)].map(
          ([, token = '', value = '']) => [token, value],
        ),
      ) as Record<string, string>,
    ],
  ),
);

function paletteFor(id: string): Record<string, string> {
  const palette = palettes[id];
  if (!palette) throw new Error(`Missing palette ${id}`);
  return palette;
}
function color(palette: Record<string, string>, token: string): string {
  const value = palette[token];
  if (!value || !/^#[\da-f]{6}$/i.test(value))
    throw new Error(`Invalid token ${token}`);
  return value;
}
function luminance(hex: string): number {
  const [r = 0, g = 0, b = 0] = (hex.match(/[\da-f]{2}/gi) ?? []).map(
    (value) => {
      const channel = parseInt(value, 16) / 255;
      return channel <= 0.04045
        ? channel / 12.92
        : ((channel + 0.055) / 1.055) ** 2.4;
    },
  );
  return r * 0.2126 + g * 0.7152 + b * 0.0722;
}
function contrast(a: string, b: string): number {
  const first = luminance(a),
    second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

const backgrounds = [
  'canvas',
  'sidebar',
  'surface',
  'panel',
  'hover',
  'accent-wash',
  'control',
  'control-hover',
];

describe('five dark themes', () => {
  it('defines exactly the allowed stable IDs and a distinct native default', () => {
    expect(themes.map(({ id }) => id)).toEqual([
      'echelon',
      'amarr',
      'caldari',
      'gallente',
      'minmatar',
    ]);
    expect(defaultTheme).toBe('echelon');
    expect(new Set(themes.map(({ name }) => name)).size).toBe(5);
    expect(Object.keys(palettes)).toEqual(themes.map(({ id }) => id));
    expect(
      new Set(Object.values(palettes).map((p) => color(p, 'accent'))).size,
    ).toBe(5);
    expect(
      new Set(Object.values(palettes).map((p) => color(p, 'canvas'))).size,
    ).toBe(5);
  });

  it.each([undefined, null, '', 'unknown', 'AMARR', '__proto__', 42])(
    'recovers invalid theme %j to ECHELON',
    (value) => {
      expect(resolveTheme(value)).toBe(defaultTheme);
    },
  );

  it('covers every shared shell color through complete semantic palettes', () => {
    expect(shellCss).not.toMatch(/#[\da-f]{3,8}\b|\brgba?\(|\bwhite\b/i);
    const used = [...shellCss.matchAll(/var\(--([\w-]+)\)/g)].map(
      ([, token = '']) => token,
    );
    const keys = Object.keys(paletteFor('echelon')).sort();
    for (const palette of Object.values(palettes)) {
      expect(Object.keys(palette).sort()).toEqual(keys);
      for (const token of used) expect(palette[token], token).toBeDefined();
    }
    expect(shellCss).toMatch(/color-scheme:\s*dark/);
    expect(shellCss).not.toMatch(/\banimation\s*:|\btransition\s*:/);
  });

  it.each(themes)(
    '$name preserves dark surfaces and accessible role contrast',
    ({ id }) => {
      const p = paletteFor(id);
      for (const bg of backgrounds) {
        expect(luminance(color(p, bg)), `${id} ${bg} is dark`).toBeLessThan(
          0.1,
        );
        for (const fg of ['text', 'muted', 'secondary-text', 'error']) {
          expect(
            contrast(color(p, fg), color(p, bg)),
            `${id} ${fg}/${bg}`,
          ).toBeGreaterThanOrEqual(4.5);
        }
        expect(
          contrast(color(p, 'focus'), color(p, bg)),
          `${id} focus/${bg}`,
        ).toBeGreaterThanOrEqual(3);
      }
      for (const bg of ['canvas', 'sidebar', 'surface', 'panel']) {
        expect(
          contrast(color(p, 'accent'), color(p, bg)),
          `${id} accent/${bg}`,
        ).toBeGreaterThanOrEqual(4.5);
      }
      for (const bg of [
        'control',
        'control-hover',
        'accent-wash',
        'sidebar',
        'surface',
      ]) {
        expect(
          contrast(color(p, 'control-border'), color(p, bg)),
          `${id} control border/${bg}`,
        ).toBeGreaterThanOrEqual(3);
      }
      expect(
        contrast(color(p, 'accent-text'), color(p, 'accent-wash')),
      ).toBeGreaterThanOrEqual(4.5);
      expect(
        contrast(color(p, 'on-accent'), color(p, 'accent')),
      ).toBeGreaterThanOrEqual(4.5);
    },
  );

  it('retains native focus styling and an independent mobile selector', () => {
    expect(shellCss).toMatch(/select:focus-visible,[^{]+\{[^}]*var\(--focus\)/);
    const mobile = shellCss.slice(
      shellCss.lastIndexOf('@media (max-width: 620px)'),
    );
    expect(mobile).toMatch(/\.appearance\s*\{\s*display:\s*grid/);
    expect(mobile).toContain('minmax(0, 2fr)');
    expect(mobile).not.toContain('display: none');
  });
});
