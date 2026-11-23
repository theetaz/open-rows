// SPDX-License-Identifier: Apache-2.0
import { EditorView } from '@codemirror/view';
import { iconGeometry } from '@openrows/ui/icons';
import type { IconName } from '@openrows/ui/icons';

const SVG = 'http://www.w3.org/2000/svg';

export function paragraph(role: 'alert' | 'status', text: string): HTMLElement {
  const element = document.createElement('p');
  element.setAttribute('role', role);
  element.textContent = text;
  return element;
}

/** A capsule push button; `prominent` is the accent-filled default action. */
export function button(
  label: string,
  action: () => void,
  look: 'plain' | 'prominent' | 'close' = 'plain',
): HTMLButtonElement {
  const element = document.createElement('button');
  element.type = 'button';
  element.className = `cm-inline-button cm-inline-${look}`;
  if (look === 'close') {
    element.setAttribute('aria-label', label);
    element.title = label;
    element.append(icon('close'));
  } else element.textContent = label;
  element.addEventListener('click', action);
  return element;
}

export function icon(name: IconName): SVGSVGElement {
  const svg = document.createElementNS(SVG, 'svg');
  const { path: d, box } = iconGeometry(name);
  svg.setAttribute('viewBox', `0 0 ${String(box)} ${String(box)}`);
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('class', 'cm-inline-icon');
  const path = document.createElementNS(SVG, 'path');
  path.setAttribute('d', d);
  svg.append(path);
  return svg;
}

/** Proposed SQL as added lines; its text content stays the exact proposal. */
export function addedLines(text: string): HTMLElement {
  const code = document.createElement('pre');
  code.className = 'cm-inline-proposal';
  code.setAttribute('aria-label', 'Proposed SQL');
  const lines = text.split('\n');
  lines.forEach((line, index) => {
    const row = document.createElement('span');
    row.className = 'cm-inline-added';
    row.textContent = index < lines.length - 1 ? `${line}\n` : line;
    code.append(row);
  });
  return code;
}

/** The review bar and diff use the kit's composer, bar and code recipes (docs/20 §5.1). */
export const inlineTheme = EditorView.baseTheme({
  '.cm-inline-review': {
    margin: '6px 12px 6px 0',
    whiteSpace: 'normal',
    fontFamily: 'var(--or-font-ui)',
    fontSize: 'var(--or-text-13)',
    lineHeight: '18px',
    color: 'var(--or-text)',
  },
  '.cm-inline-review p': { margin: '0' },
  '.cm-inline-prompt': {
    display: 'flex',
    alignItems: 'center',
    gap: '6px',
    padding: '5px 5px 5px 10px',
    borderRadius: 'var(--or-radius-card)',
    background: 'var(--or-control-fill)',
  },
  '.cm-inline-prompt:focus-within': {
    outline: '3px solid var(--or-field-focus)',
  },
  '.cm-inline-prompt input': {
    flex: '1',
    minWidth: '0',
    padding: '2px 0',
    border: '0',
    outline: 'none',
    background: 'none',
    color: 'inherit',
    font: 'inherit',
  },
  '.cm-inline-prompt input::placeholder': { color: 'var(--or-placeholder)' },
  '.cm-inline-icon': {
    width: '16px',
    height: '16px',
    flex: 'none',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: '1.5',
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
    color: 'var(--or-secondary)',
  },
  '.cm-inline-tag': {
    flex: 'none',
    padding: '0 5px',
    borderRadius: '4px',
    background: 'var(--or-control-fill)',
    color: 'var(--or-secondary)',
    fontSize: 'var(--or-text-11)',
    lineHeight: '16px',
  },
  '.cm-inline-bar': {
    display: 'flex',
    flexWrap: 'wrap',
    alignItems: 'center',
    gap: '8px',
    minHeight: '36px',
    fontSize: 'var(--or-text-12)',
  },
  '.cm-inline-bar strong': { fontWeight: 'var(--or-weight-semibold)' },
  '.cm-inline-bar p': { flex: '1', color: 'var(--or-secondary)' },
  '.cm-inline-bar p[role="alert"]': { color: 'var(--or-danger-fg)' },
  '.cm-inline-button': {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    height: 'var(--or-control-height-compact)',
    padding: '0 var(--or-control-padding-compact)',
    border: '0',
    borderRadius: 'var(--or-radius-capsule)',
    background: 'var(--or-control-fill)',
    color: 'var(--or-text)',
    font: 'inherit',
    fontSize: 'var(--or-text-12)',
    fontWeight: 'var(--or-weight-medium)',
    cursor: 'default',
  },
  '.cm-inline-button:focus-visible': {
    outline: 'var(--or-focus-width) solid var(--or-focus)',
    outlineOffset: '1px',
  },
  '.cm-inline-prominent': {
    background: 'var(--or-accent-fill)',
    color: 'var(--or-on-accent)',
  },
  '.cm-inline-close': {
    width: 'var(--or-control-height-compact)',
    padding: '0',
    borderRadius: 'var(--or-radius-menu-item)',
    background: 'transparent',
  },
  '.cm-inline-review pre': {
    whiteSpace: 'pre-wrap',
    maxHeight: '240px',
    overflow: 'auto',
    margin: '0',
    padding: '4px 0',
    fontFamily: 'var(--or-font-editor)',
    lineHeight: '20px',
  },
  '.cm-inline-added': {
    display: 'block',
    position: 'relative',
    paddingLeft: '20px',
    background: 'var(--or-success-bg)',
  },
  '.cm-inline-added::before': {
    content: '"+"',
    position: 'absolute',
    left: '6px',
    color: 'var(--or-success-fg)',
  },
  '.cm-inline-target': {
    outline: '1px dashed var(--or-accent)',
  },
  '.cm-inline-original': {
    textDecoration: 'line-through',
    background: 'var(--or-danger-bg)',
    color: 'var(--or-secondary)',
  },
});
