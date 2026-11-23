// SPDX-License-Identifier: Apache-2.0
import { light, dark, highContrast } from './colours.ts';
import { geometry } from './geometry.ts';

type Colour = readonly [number, number, number, number];
function parse(value: string): Colour {
  if (value.startsWith('#'))
    return [
      parseInt(value.slice(1, 3), 16),
      parseInt(value.slice(3, 5), 16),
      parseInt(value.slice(5, 7), 16),
      1,
    ];
  const values = value.match(/[\d.]+/g)?.map(Number);
  if (!values || values.length !== 4)
    throw new Error(`Unsupported colour: ${value}`);
  return [values[0] ?? 0, values[1] ?? 0, values[2] ?? 0, values[3] ?? 1];
}
function composite(foreground: Colour, background: Colour): Colour {
  const alpha = foreground[3];
  const channel = (index: 0 | 1 | 2): number =>
    foreground[index] * alpha + background[index] * (1 - alpha);
  return [channel(0), channel(1), channel(2), 1];
}
function luminance(colour: Colour): number {
  const linear = colour.slice(0, 3).map((channel) => {
    const value = channel / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  return (
    (linear[0] ?? 0) * 0.2126 +
    (linear[1] ?? 0) * 0.7152 +
    (linear[2] ?? 0) * 0.0722
  );
}
function colourRatio(foreground: Colour, background: Colour): number {
  const first = luminance(foreground);
  const second = luminance(background);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}
function ratio(foreground: string, background: Colour): number {
  return colourRatio(composite(parse(foreground), background), background);
}
/** WCAG ratio for a foreground composited over an opaque sRGB background. */
export function contrastRatio(
  foreground: string,
  background: string,
  base?: string,
): number {
  const fill = base
    ? composite(parse(background), parse(base))
    : parse(background);
  return ratio(foreground, fill);
}
type Row = { theme: string; pair: string; ratio: number; minimum: number };
/** Text is 4.5:1; NULL markers, focus outlines and icons are non-text marks at 3:1. */
const MINIMUM: Partial<Record<keyof typeof light, number>> = {
  placeholder: 3,
  focus: 3,
  tertiary: 3,
};
/** Solid content surfaces and row states: text, NULL markers and status pills. */
function contentSurfaces(palette: typeof light): Record<string, Colour> {
  const page = parse(palette.page);
  const window = parse(palette.window);
  return {
    page,
    window,
    panel: composite(parse(palette.panel), page),
    'panel/window': composite(parse(palette.panel), window),
    'panel-solid': parse(palette['panel-solid']),
    group: parse(palette.group),
    sheet: composite(parse(palette['glass-sheet']), page),
    float: composite(parse(palette['glass-float']), page),
    ...Object.fromEntries(
      (
        [
          'row-stripe',
          'row-inserted',
          'row-deleted',
          'cell-edited',
          'accent-soft',
        ] as const
      ).map((state) => [state, composite(parse(palette[state]), window)]),
    ),
  };
}
/** Control fills on the surfaces controls sit on. */
function controlSurfaces(palette: typeof light): Record<string, Colour> {
  const over = (key: keyof typeof light, base: Colour): Colour =>
    composite(parse(palette[key]), base);
  const page = parse(palette.page);
  const bases: Record<string, Colour> = {
    page,
    group: parse(palette.group),
    sheet: over('glass-sheet', page),
    float: over('glass-float', page),
  };
  return Object.fromEntries(
    Object.entries(bases).map(([name, base]) => [
      `fill/${name}`,
      over('control-fill', base),
    ]),
  );
}
/** Row states painted by the canvas grid, which draws plain text, never pills. */
const GRID_ONLY = new Set([
  'row-inserted',
  'row-deleted',
  'cell-edited',
  'accent-soft',
]);
type Role = 'text' | 'secondary' | 'danger-fg' | 'placeholder' | 'focus';
function roleRows(
  theme: string,
  palette: typeof light,
  roles: readonly Role[],
  backgrounds: Record<string, Colour>,
): Row[] {
  return Object.entries(backgrounds).flatMap(([name, background]) =>
    roles.map((foreground) => ({
      theme,
      pair: `${foreground}/${name}`,
      ratio: ratio(palette[foreground], background),
      minimum: MINIMUM[foreground] ?? 4.5,
    })),
  );
}
export function paletteRows(theme: string, palette: typeof light): Row[] {
  const content = contentSurfaces(palette);
  const controls = controlSurfaces(palette);
  const page = parse(palette.page);
  const field = { raised: parse(palette.raised) };
  const track = composite(parse(palette['control-fill']), page);
  const nested = {
    'nested-fill/page': composite(parse(palette['control-fill']), track),
    'selected-segment': composite(parse(palette['control-active']), track),
  };
  const rows: Row[] = [
    ...roleRows(
      theme,
      palette,
      ['text', 'secondary', 'danger-fg', 'placeholder', 'focus'],
      content,
    ),
    ...roleRows(theme, palette, ['text', 'secondary', 'danger-fg'], controls),
    ...roleRows(theme, palette, ['text', 'placeholder'], field),
    ...roleRows(theme, palette, ['text'], nested),
    ...Object.entries(content)
      .filter(([name]) => !GRID_ONLY.has(name))
      .flatMap(([name, background]) =>
        badgeRows({ theme, palette, name, background }),
      ),
  ];
  for (const background of [
    'accent-fill',
    'connection-pg',
    'connection-prod',
    'connection-ch',
    'connection-sqlite',
  ] as const)
    rows.push({
      theme,
      pair: `on-accent/${background}`,
      ratio: ratio(palette['on-accent'], parse(palette[background])),
      minimum: 4.5,
    });
  rows.push(
    ...railRows(theme, palette),
    ...chromeRows(theme, palette),
    ...environmentRows(theme, palette),
  );
  return rows;
}

function chromeRows(theme: string, palette: typeof light): Row[] {
  const page = parse(palette.page);
  const over = (key: keyof typeof light, base: Colour): Colour =>
    composite(parse(palette[key]), base);
  const capsule = over('glass-fill', page);
  const pairs: [keyof typeof light, string, Colour, number][] = [
    ['label-tertiary', 'page', page, 4.5],
    ['label-tertiary', 'search', over('control-fill', page), 4.5],
    ['text', 'toolbar-capsule', capsule, 4.5],
    ['tertiary', 'page', page, 3],
    ['address-text', 'address', over('address', over('toolbar', page)), 4.5],
    ['secondary', 'address', over('address', over('toolbar', page)), 4.5],
    ['text', 'field-box', over('field-box-fill', page), 4.5],
    ['accent', 'page', page, 4.5],
    ['accent', 'group', parse(palette.group), 4.5],
  ];
  return pairs.map(([foreground, name, background, minimum]) => ({
    theme,
    pair: `${foreground}/${name}`,
    ratio: ratio(palette[foreground], background),
    minimum,
  }));
}
/**
 * The environment colour in the window chrome: the context bar's tag is text
 * on its badge fill over a solid panel, so the glass behind it cannot weaken
 * it; the tint strip and tab marks are graphics (WCAG 1.4.11).
 */
function environmentRows(theme: string, palette: typeof light): Row[] {
  const toolbar = composite(parse(palette.toolbar), parse(palette.page));
  const strip = parse(palette['panel-solid']);
  const marks: [string, Colour][] = [
    ['toolbar', toolbar],
    ['tab-strip', strip],
    ['tab-active', composite(parse(palette['control-active']), strip)],
    ['tab-hover', composite(parse(palette['control-fill']), strip)],
  ];
  return (['local', 'dev', 'staging', 'prod'] as const).flatMap((tag) => {
    const fg = palette[`${tag}-fg`];
    const fill = parse(palette[`${tag}-bg`]);
    return [
      {
        theme,
        pair: `env-tag-${tag}/panel-solid`,
        ratio: ratio(fg, composite(fill, strip)),
        minimum: 4.5,
      },
      ...marks.map(([name, base]) => ({
        theme,
        pair: `env-mark-${tag}/${name}`,
        ratio: ratio(fg, base),
        minimum: 3,
      })),
    ];
  });
}
/** Full WCAG sRGB coverage of text/focus pairs, composited glass and badge fills. */
export function contrastRows(): Row[] {
  return [
    ...paletteRows('light', light),
    ...paletteRows('dark', dark),
    ...paletteRows('high-contrast-light', { ...light, ...highContrast.light }),
    ...paletteRows('high-contrast-dark', { ...dark, ...highContrast.dark }),
  ];
}

function badgeRows({
  theme,
  palette,
  name,
  background,
}: {
  theme: string;
  palette: typeof light;
  name: string;
  background: Colour;
}): Row[] {
  const rows: Row[] = [];
  for (const tone of [
    'warning',
    'success',
    'info',
    'neutral',
    'danger',
    'local',
    'dev',
    'staging',
    'prod',
  ] as const) {
    rows.push({
      theme,
      pair: `${tone}/${name}`,
      ratio: ratio(
        palette[`${tone}-fg`],
        composite(parse(palette[`${tone}-bg`]), background),
      ),
      minimum: 4.5,
    });
  }
  return rows;
}

function railRows(theme: string, palette: typeof light): Row[] {
  const opacity = Number(geometry['rail-inactive-opacity']);
  const page = parse(palette.page);
  const faded = (value: string): Colour => {
    const colour = parse(value);
    return composite([colour[0], colour[1], colour[2], opacity], page);
  };
  return (['connection-pg', 'connection-ch', 'connection-sqlite'] as const).map(
    (engine) => ({
      theme,
      pair: `rail-inactive/${engine}`,
      ratio: colourRatio(faded(palette['on-accent']), faded(palette[engine])),
      minimum: 4.5,
    }),
  );
}
