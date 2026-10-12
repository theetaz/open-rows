// SPDX-License-Identifier: Apache-2.0
// Standalone SVG of a chart section: fixed colours (no theme tokens, no
// scripts, no external resources), gaps drawn as breaks and tick marks.
import { downsample } from '../boards/boardData';
import { escapeHtml } from './escape';
import { numberText, roundValue } from './model';
import type { ChartPoint, ChartSection } from './model';

export const CHART_SIZE = { width: 640, height: 260 } as const;
const PLOT = { left: 56, top: 36, right: 624, bottom: 216 } as const;
const COLOURS = [
  '#2563EB',
  '#52525B',
  '#16A34A',
  '#D97706',
  '#9333EA',
  '#DC2626',
  '#0891B2',
  '#A1A1AA',
];
const TEXT = 'font-family="-apple-system, system-ui, sans-serif"';

type Scale = { x: (x: number) => number; y: (y: number) => number };
type Extent = [number, number];

function extent(values: number[], baseline: boolean): Extent {
  const finite = values.filter(Number.isFinite);
  if (baseline) finite.push(0);
  if (!finite.length) return [0, 1];
  const low = Math.min(...finite);
  const high = Math.max(...finite);
  return low === high ? [low, low + 1] : [low, high];
}

function scaleOf(xs: Extent, ys: Extent): Scale {
  const width = PLOT.right - PLOT.left;
  const height = PLOT.bottom - PLOT.top;
  return {
    x: (x) => PLOT.left + ((x - xs[0]) / (xs[1] - xs[0] || 1)) * width,
    y: (y) => PLOT.bottom - ((y - ys[0]) / (ys[1] - ys[0])) * height,
  };
}

/** Line segments that break at gaps and at missing collection intervals. */
function linePath(
  points: ChartPoint[],
  scale: Scale,
  breakAfter: number,
): string {
  let path = '';
  let previous: number | null = null;
  for (const point of points) {
    if (point.y === null) {
      previous = null;
      continue;
    }
    const move = previous === null || point.x - previous > breakAfter;
    path += `${move ? 'M' : 'L'}${scale.x(point.x).toFixed(1)},${scale.y(point.y).toFixed(1)} `;
    previous = point.x;
  }
  return path.trim();
}

function marks(
  section: ChartSection,
  scale: Scale,
  index: number,
  points: ChartPoint[],
): string {
  const colour = COLOURS[index % COLOURS.length] ?? '#52525B';
  const valued = points.filter((point) => point.y !== null);
  if (section.mark === 'scatter' || section.mark === 'bar') {
    const width = Math.max(
      1,
      ((PLOT.right - PLOT.left) / Math.max(1, points.length)) * 0.6,
    );
    return valued
      .map((point) => {
        const x = scale.x(point.x);
        const y = scale.y(point.y ?? 0);
        if (section.mark === 'scatter')
          return `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="2" fill="${colour}"/>`;
        const base = scale.y(0);
        return `<rect x="${(x - width / 2).toFixed(1)}" y="${Math.min(y, base).toFixed(1)}" width="${width.toFixed(1)}" height="${Math.abs(base - y).toFixed(1)}" fill="${colour}"/>`;
      })
      .join('');
  }
  const breakAfter = section.cadenceMs ? section.cadenceMs * 1.5 : Infinity;
  const path = linePath(points, scale, breakAfter);
  return path
    ? `<path d="${path}" fill="none" stroke="${colour}" stroke-width="1.5"/>`
    : '';
}

function gapTicks(points: ChartPoint[], scale: Scale): string {
  return points
    .filter((point) => point.y === null && Number.isFinite(point.x))
    .map(
      (point) =>
        `<line x1="${scale.x(point.x).toFixed(1)}" x2="${scale.x(point.x).toFixed(1)}" y1="${String(PLOT.bottom + 2)}" y2="${String(PLOT.bottom + 7)}" stroke="#A1A1AA"/>`,
    )
    .join('');
}

function text(x: number, y: number, value: string, extra = ''): string {
  return `<text x="${String(x)}" y="${String(y)}" ${TEXT} font-size="10" fill="#71717A"${extra}>${escapeHtml(value)}</text>`;
}

function axes(section: ChartSection, ys: Extent, points: ChartPoint[]): string {
  const first = points[0]?.label ?? '';
  const last = points.at(-1)?.label ?? '';
  const unit = section.unit ? ` (${section.unit})` : '';
  return [
    `<rect x="${String(PLOT.left)}" y="${String(PLOT.top)}" width="${String(PLOT.right - PLOT.left)}" height="${String(PLOT.bottom - PLOT.top)}" fill="none" stroke="#E4E4E7"/>`,
    text(
      PLOT.left - 4,
      PLOT.top + 4,
      numberText(roundValue(ys[1])),
      ' text-anchor="end"',
    ),
    text(
      PLOT.left - 4,
      PLOT.bottom,
      numberText(roundValue(ys[0])),
      ' text-anchor="end"',
    ),
    text(PLOT.left, PLOT.bottom + 20, first),
    text(PLOT.right, PLOT.bottom + 20, last, ' text-anchor="end"'),
    `<text x="12" y="20" ${TEXT} font-size="13" fill="#18181B">${escapeHtml(section.title + unit)}</text>`,
  ].join('');
}

function legend(section: ChartSection): string {
  if (section.series.length < 2) return '';
  return section.series
    .map((series, index) => {
      const x = PLOT.left + index * 96;
      const colour = COLOURS[index % COLOURS.length] ?? '#52525B';
      return `<rect x="${String(x)}" y="240" width="8" height="8" fill="${colour}"/>${text(x + 12, 248, series.name.slice(0, 14))}`;
    })
    .join('');
}

/** Bounded standalone SVG: at most 600 drawn points per series. */
export function chartSvg(section: ChartSection): string {
  const series = section.series.map(
    (s) => downsample(s.points, 300) as ChartPoint[],
  );
  const all = series.flat();
  const baseline = section.mark === 'bar' || section.mark === 'area';
  const ys = extent(
    all.flatMap((p) => (p.y === null ? [] : [p.y])),
    baseline,
  );
  const scale = scaleOf(
    extent(
      all.map((p) => p.x),
      false,
    ),
    ys,
  );
  const { width, height } = CHART_SIZE;
  const description = `${section.title}: ${section.evidence.scope} scope. Missing observations are gaps, never zeros.`;
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${String(width)}" height="${String(height)}" viewBox="0 0 ${String(width)} ${String(height)}" role="img">`,
    `<title>${escapeHtml(section.title)}</title><desc>${escapeHtml(description)}</desc>`,
    `<rect width="${String(width)}" height="${String(height)}" fill="#FFFFFF"/>`,
    axes(section, ys, series[0] ?? []),
    series
      .map((points, index) => marks(section, scale, index, points))
      .join(''),
    gapTicks(all, scale),
    legend(section),
    '</svg>',
  ].join('');
}
