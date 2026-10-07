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
        : 'mock';
  return readFileSync(`apps/desktop/src/${folder}/${file}.module.css`, 'utf8');
}
it('content panels are borderless with distinct theme surfaces and concentric radii', () => {
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
  }
  for (const palette of [light, dark]) {
    expect(
      new Set([palette.page, palette['panel-solid'], palette.window]).size,
    ).toBe(3);
  }
  expect(css('Rail')).toContain('background: var(--or-page)');
  expect(css('Query')).toContain('background: var(--or-window)');
  expect(css('Query')).toContain('border-radius: var(--or-radius-group)');
  expect(geometry['rail-tile-size']).toBe('36px');
  expect(geometry['radius-field']).toBe('10px');
});
