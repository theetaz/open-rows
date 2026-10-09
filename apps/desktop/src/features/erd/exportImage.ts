// SPDX-License-Identifier: Apache-2.0
import { saveDiagram } from '@openrows/ipc';
import { DIAGRAM_STYLE } from './diagramStyle';
import { svgToPng } from './rasterize';
import type { Size } from './viewport';

const SVG_NS = 'http://www.w3.org/2000/svg';

export type ExportFormat = 'svg' | 'png';

/** Replace every `var(--name)` with its value from `lookup`. */
export function resolveTokens(
  text: string,
  lookup: (name: string) => string,
): string {
  return text.replace(/var\((--[\w-]+)\)/g, (match, name: string) => {
    const value = lookup(name).trim();
    return value || match;
  });
}

/**
 * Standalone SVG of the whole diagram: the drawn content without search
 * state, on the page background, with token colours resolved where it is
 * drawn (so the file matches the current theme).
 */
export function standaloneSvg(content: SVGGElement, size: Size): string {
  const svg = document.createElementNS(SVG_NS, 'svg');
  const width = String(Math.ceil(size.width));
  const height = String(Math.ceil(size.height));
  svg.setAttribute('xmlns', SVG_NS);
  svg.setAttribute('width', width);
  svg.setAttribute('height', height);
  svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
  const style = document.createElementNS(SVG_NS, 'style');
  style.textContent = `${DIAGRAM_STYLE}.erd-page { fill: var(--or-page); }`;
  const page = document.createElementNS(SVG_NS, 'rect');
  page.setAttribute('class', 'erd-page');
  page.setAttribute('width', width);
  page.setAttribute('height', height);
  const copy = content.cloneNode(true) as SVGGElement;
  copy.removeAttribute('data-searching');
  for (const node of copy.querySelectorAll('[data-match]'))
    node.removeAttribute('data-match');
  for (const node of copy.querySelectorAll('[tabindex]'))
    node.removeAttribute('tabindex');
  svg.append(style, page, copy);
  const computed = getComputedStyle(content);
  return resolveTokens(new XMLSerializer().serializeToString(svg), (name) =>
    computed.getPropertyValue(name),
  );
}

/** Export the diagram as SVG or PNG through the native save panel. */
export async function exportDiagram(
  format: ExportFormat,
  name: string,
  drawing: { content: SVGGElement; size: Size },
): Promise<string | null> {
  const svg = standaloneSvg(drawing.content, drawing.size);
  if (format === 'svg') return saveDiagram(name, { format: 'svg', text: svg });
  const bytes = await svgToPng(svg, drawing.size);
  return saveDiagram(name, { format: 'png', bytes: Array.from(bytes) });
}
