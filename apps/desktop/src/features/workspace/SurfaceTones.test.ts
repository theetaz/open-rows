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

it('v4 materials: tinted window, veiled content, a floating clear-glass sidebar', () => {
  const frame = css('features/workspace', 'Frame');
  expect(rule(frame, 'workspace')).toContain(
    'background: var(--or-window-tint);',
  );
  expect(frame).toMatch(
    /aside\[aria-label='Sidebar'\] \{\s*background: transparent;/,
  );
  expect(rule(frame, 'main')).toContain('background: var(--or-window-veil);');
  expect(rule(frame, 'body')).not.toMatch(/background/);
  const sidebar = rule(frame, 'sidebar');
  expect(sidebar).toContain('background: var(--or-glass-sidebar);');
  expect(sidebar).toContain('border-radius: var(--or-radius-panel);');
  // The veil round the panel is its spread shadow, never a fill under it.
  expect(sidebar).toContain('0 0 0 60px var(--or-window-veil)');
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
  expect(geometry['radius-panel']).toBe('11px');
  expect(geometry['radius-field']).toBe('7px');
});
