// SPDX-License-Identifier: Apache-2.0
import { render } from '@testing-library/react';
import { expect, it } from 'vitest';
import { engineTiles } from '../../tokens/engines';
import { EngineIcon, isEngineId } from './EngineIcon';
import { engineGlyphs } from './engineGlyphs';

it('has a glyph and tile colours for every engine', () => {
  expect(Object.keys(engineTiles).sort()).toEqual(
    Object.keys(engineGlyphs).sort(),
  );
  expect(Object.keys(engineGlyphs)).toHaveLength(22);
  for (const glyph of Object.values(engineGlyphs))
    expect(glyph.length).toBeGreaterThan(0);
});

it('draws a decorative tile at the requested size', () => {
  const { container } = render(<EngineIcon engine="postgres" size="lg" />);
  const tile = container.firstElementChild;
  expect(tile).toHaveAttribute('aria-hidden', 'true');
  expect(tile).toHaveAttribute('data-size', 'lg');
  expect(tile?.getAttribute('style')).toContain('--engine-bg');
  expect(container.querySelectorAll('path')).toHaveLength(3);
});

it('recognises stored engine ids', () => {
  expect(isEngineId('libsql-file')).toBe(true);
  expect(isEngineId('nosuch')).toBe(false);
});
