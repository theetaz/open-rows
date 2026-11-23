// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { dark, light, geometry } from '@openrows/ui/tokens';

function css(folder: string, file: string): string {
  return readFileSync(`apps/desktop/src/${folder}/${file}.module.css`, 'utf8');
}
/** Declarations of the first rule whose selector is exactly `.name`. */
function rule(style: string, name: string): string {
  return style.match(new RegExp(`\\.${name} \\{([^}]+)\\}`))?.[1] ?? '';
}

it('glass only floats: the sidebar is a flat edge-to-edge veil, content is solid', () => {
  const frame = css('features/workspace', 'Frame');
  const sidebar = rule(frame, 'sidebar');
  expect(sidebar).not.toMatch(/glass|border-radius|margin|box-shadow/);
  expect(frame).toMatch(
    /aside\[aria-label='Sidebar'\] \{\s*background: var\(--or-window-veil\);/,
  );
  for (const name of ['main', 'body'])
    expect(rule(frame, name)).toContain('background: var(--or-page);');
  // Panes inside never paint glass or outlines of their own.
  for (const [file, name] of [
    ['Sidebar', 'sidebar'],
    ['Inspector', 'inspector'],
  ] as const) {
    const body = rule(css('features/workspace', file), name);
    expect(body).toContain('border: 0;');
    expect(body).toContain('box-shadow: none;');
    expect(body).not.toMatch(/glass/);
  }
  expect(css('features/query', 'Query')).toContain(
    'background: var(--or-window)',
  );
});

it('toolbar capsules are glass; the toolbar and tab strip are solid', () => {
  expect(rule(css('features/workspace', 'Workspace'), 'toolbar')).toContain(
    'background: var(--or-toolbar);',
  );
  const panels = readFileSync(
    'packages/ui/src/components/Panels.module.css',
    'utf8',
  );
  expect(rule(panels, 'toolbar')).toContain(
    'background: var(--or-glass-fill);',
  );
  for (const palette of [light, dark]) {
    expect(palette.toolbar).toBe(palette.page);
    expect(palette.window).toBe(palette.page);
    expect(palette['panel-solid']).not.toBe(palette.page);
  }
  expect(geometry['radius-panel']).toBe('14px');
  expect(geometry['radius-field']).toBe('8px');
});
