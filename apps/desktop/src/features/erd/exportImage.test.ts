// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { pngScale } from './rasterize';
import { resolveTokens, standaloneSvg } from './exportImage';

it('resolves token references and keeps unknown ones', () => {
  const lookup = (name: string): string =>
    name === '--or-text' ? ' #18181B ' : '';
  expect(resolveTokens('fill: var(--or-text); x: var(--or-gone)', lookup)).toBe(
    'fill: #18181B; x: var(--or-gone)',
  );
});

it('exports the drawing without search state, on the page colour', () => {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.innerHTML =
    '<g data-diagram="" data-searching=""><g class="erd-table" data-match="" tabindex="0"><text>orders</text></g></g>';
  document.body.append(svg);
  const content = svg.querySelector('g');
  if (!(content instanceof SVGGElement)) throw new Error('no content');
  // jsdom does not inherit custom properties; WebKit reads them from :root.
  content.style.setProperty('--or-page', '#F4F4F5');
  content.style.setProperty('--or-text', '#18181B');
  const text = standaloneSvg(content, { width: 300.4, height: 120 });
  expect(text).toMatch(/^<svg[^>]+width="301" height="120"/);
  expect(text).toContain('viewBox="0 0 301 120"');
  expect(text).toContain('.erd-page { fill: #F4F4F5; }');
  expect(text).toContain('fill: #18181B');
  expect(text).toContain('<text>orders</text>');
  expect(text.replace(/<style>[^<]*<\/style>/, '')).not.toMatch(
    /data-searching|data-match|tabindex/,
  );
  svg.remove();
});

it('rasterises at 2x unless the canvas would be too large', () => {
  expect(pngScale({ width: 1000, height: 800 })).toBe(2);
  expect(pngScale({ width: 8000, height: 8000 })).toBeCloseTo(0.5);
  expect(pngScale({ width: 40_000, height: 100 })).toBeCloseTo(16_384 / 40_000);
});
