import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { defaultTheme, resolveTheme, themes } from './themes';

const css = readFileSync('src/themes/themes.css', 'utf8');
const shellCss = readFileSync('src/shell/styles.css', 'utf8');
function tokens(block: string): Record<string, string> {
  return Object.fromEntries(
    [...block.matchAll(/--([\w-]+):\s*(#[\da-f]{6})/g)].map(
      ([, token = '', value = '']) => [token, value],
    ),
  );
}
const base = tokens(css.match(/:root\s*\{([^}]+)\}/)?.[1] ?? '');
const highlights = ['accent', 'accent-text', 'accent-wash', 'focus'];
const overrides: Record<string, Record<string, string>> = Object.fromEntries(
  [...css.matchAll(/:root\[data-theme='([a-z]+)'\]\s*\{([^}]+)\}/g)].map(
    ([, id = '', block = '']) => [id, tokens(block)],
  ),
);
const palettes: Record<string, Record<string, string>> = Object.fromEntries(
  Object.entries(overrides).map(([id, values]) => [id, { ...base, ...values }]),
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
    ).toBe(1);
  });

  it('inherits the locked neutral foundation with only explicit highlight overrides', () => {
    expect(base.canvas).toBe('#191919');
    expect(base.sidebar).toBe('#202020');
    expect(base.surface).toBe('#242424');
    expect(base.panel).toBe('#242424');
    const neutral = [
      'canvas',
      'sidebar',
      'surface',
      'panel',
      'text',
      'muted',
      'secondary-text',
      'hover',
      'line',
      'control',
      'control-hover',
      'control-border',
      'on-accent',
    ];
    for (const { id } of themes) {
      expect(Object.keys(overrides[id] ?? {}).sort()).toEqual(
        [...highlights].sort(),
      );
      const palette = paletteFor(id);
      for (const token of Object.keys(base).filter(
        (key) => !highlights.includes(key),
      )) {
        expect(color(palette, token), `${id} invariant ${token}`).toBe(
          base[token],
        );
      }
      for (const token of neutral) {
        const hex = color(palette, token).slice(1);
        expect(hex.slice(0, 2)).toBe(hex.slice(2, 4));
        expect(hex.slice(2, 4)).toBe(hex.slice(4, 6));
      }
    }
    // Reject extra selectors/media overrides that could silently defeat the modeled cascade.
    expect(
      css
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/:root(?:\[data-theme='[a-z]+'\])?\s*\{[^}]*\}/g, '')
        .trim(),
    ).toBe('');
    expect(shellCss).toMatch(
      /\.runtime-connected\s*\{\s*color: var\(--status\)/,
    );
    expect(shellCss).toMatch(
      /\.status-dot\s*\{[^}]*background: var\(--status\)/,
    );
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
        for (const fg of [
          'text',
          'muted',
          'secondary-text',
          'error',
          'status',
        ]) {
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
        contrast(color(p, 'accent'), color(p, 'accent-wash')),
      ).toBeGreaterThanOrEqual(3);
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
