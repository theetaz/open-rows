// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { generateCss } from '../../scripts/token-css';
import { contrastRows, contrastRatio } from './contrast';

const css = readFileSync('packages/ui/src/tokens/tokens.css', 'utf8');
describe('generated tokens', () => {
  it('computes known WCAG endpoints and alpha composition', () => {
    expect(contrastRatio('#FFFFFF', '#000000')).toBe(21);
    expect(contrastRatio('#FFFFFF', '#FFFFFF')).toBe(1);
    expect(contrastRatio('rgba(0,0,0,0.5)', '#FFFFFF')).toBeCloseTo(3.97665, 4);
  });
  it('matches the deterministic TypeScript generator byte for byte', () => {
    expect(css).toBe(generateCss());
    expect(generateCss()).toBe(generateCss());
  });
  it('supports OS theme, contrast, motion, transparency and a solid override', () => {
    for (const preference of [
      'prefers-color-scheme: dark',
      'prefers-contrast: more',
      'prefers-reduced-motion: reduce',
      'prefers-reduced-transparency: reduce',
    ])
      expect(css).toContain(`@media (${preference})`);
    expect(css).toMatch(
      /\[data-transparency="reduced"\]\s*\{[^}]*--or-panel: var\(--or-panel-solid\);[^}]*--or-glass-panel: none;[^}]*--or-glass-toolbar: none;/,
    );
    expect(css).toContain(
      'animation: none !important; transition: none !important;',
    );
    const panelCss = readFileSync(
      'packages/ui/src/components/Panels.module.css',
      'utf8',
    );
    expect(panelCss).toContain('background: var(--or-panel)');
    expect(panelCss).toContain('backdrop-filter: var(--or-glass-panel)');
  });
  it.each(contrastRows())('$theme $pair ≥ $minimum', (row) => {
    expect(
      row.ratio,
      `${row.theme} ${row.pair}: ${row.ratio.toFixed(2)}`,
    ).toBeGreaterThanOrEqual(row.minimum);
  });
});
