// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { dark, light, geometry } from '@openrows/ui/tokens';
function css(file: string): string {
  const folder = file === 'Rail' ? 'features/connections' : 'mock';
  return readFileSync(`apps/desktop/src/${folder}/${file}.module.css`, 'utf8');
}
it('workspace surfaces are borderless and use ordered theme tones with concentric radii', () => {
  for (const [file, selector] of [
    ['Rail', 'rail'],
    ['Sidebar', 'sidebar'],
    ['Inspector', 'inspector'],
    ['Content', 'content'],
  ]) {
    const style = css(file ?? '');
    const rule = style.match(
      new RegExp(`\\.${selector ?? ''} \\{([^}]+)\\}`),
    )?.[1];
    expect(rule).toContain('border: 0;');
    expect(rule).not.toMatch(/outline:/);
  }
  for (const palette of [light, dark]) {
    const tones = [palette.page, palette['panel-solid'], palette.window].map(
      (colour) => parseInt(colour.slice(1), 16),
    );
    expect(tones[0]).toBeLessThan(tones[1] ?? 0);
    expect(tones[1]).toBeLessThan(tones[2] ?? 0);
  }
  expect(css('Rail')).toContain('background: var(--or-page)');
  expect(css('Content')).toContain('background: var(--or-window)');
  expect(css('Content')).toContain('border-radius: var(--or-radius-group)');
  expect(geometry['rail-tile-size']).toBe('36px');
  expect(geometry['radius-field']).toBe('10px');
});
it('rail selection uses a pill without tile rings and bottom actions have no borders', () => {
  const style = css('Rail');
  expect(style).toMatch(/\.indicator\s*\{[^}]*width: 4px/);
  expect(style).toMatch(/\[data-state='selected'\]\s*\{\s*height: 20px/);
  expect(style).toMatch(/\[data-state='hover'\]\s*\{\s*height: 8px/);
  expect(style).not.toContain('outline: 2px');
  expect(style).toMatch(/\.rail > button\s*\{\s*border: 0/);
  expect(style).toContain('border: 2px solid var(--or-page)');
  expect(style).toContain('width: var(--or-dot-size)');
});
