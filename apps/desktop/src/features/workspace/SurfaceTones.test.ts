// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { dark, light, geometry } from '@openrows/ui/tokens';
function css(file: string): string {
  const folder =
    file === 'Rail'
      ? 'features/connections'
      : file === 'Query'
        ? 'features/query'
        : 'features/workspace';
  return readFileSync(`apps/desktop/src/${folder}/${file}.module.css`, 'utf8');
}
it('panes are edge-to-edge: borderless, unrounded, with a distinct content surface', () => {
  for (const [file, selector] of [
    ['Sidebar', 'sidebar'],
    ['Inspector', 'inspector'],
    ['Query', 'content'],
  ]) {
    const style = css(file ?? '');
    const rule = style.match(
      new RegExp(`\\.${selector ?? ''} \\{([^}]+)\\}`),
    )?.[1];
    expect(rule).toContain('border: 0;');
    expect(rule).not.toMatch(/outline:/);
    expect(rule).not.toMatch(/border-radius: var\(--or-radius-(group|panel)\)/);
  }
  for (const palette of [light, dark]) {
    expect(palette.window).not.toBe(palette['panel-solid']);
    expect(palette.window).not.toBe(palette.page);
  }
  expect(css('Rail')).toContain('background: var(--or-page)');
  expect(css('Query')).toContain('background: var(--or-window)');
  expect(geometry['radius-field']).toBe('6px');
});
