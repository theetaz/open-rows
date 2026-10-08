// SPDX-License-Identifier: Apache-2.0
import type { Completion } from '@codemirror/autocomplete';
import { iconPaths } from '@openrows/ui/icons';
import type { IconName } from '@openrows/ui/icons';
import type { CompletionKind } from './catalog';

const ICONS: Record<CompletionKind, IconName> = {
  keyword: 'terminal',
  table: 'table',
  view: 'eye',
  function: 'function',
  column: 'column',
  schema: 'schema',
};

const SVG = 'http://www.w3.org/2000/svg';

function isKind(type: string | undefined): type is CompletionKind {
  return type !== undefined && type in ICONS;
}

/** The kind icon of a completion row, tinted per kind by the editor theme. */
function renderIcon(completion: Completion): Node {
  const kind = isKind(completion.type) ? completion.type : 'keyword';
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('class', `cm-sqlIcon cm-sqlIcon-${kind}`);
  const path = document.createElementNS(SVG, 'path');
  path.setAttribute('d', iconPaths[ICONS[kind]]);
  svg.append(path);
  return svg;
}

/** `addToOptions` entry drawing the icon before the label. */
export const completionIcon = { render: renderIcon, position: 20 };
