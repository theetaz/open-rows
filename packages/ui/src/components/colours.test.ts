// SPDX-License-Identifier: Apache-2.0
import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { expect, it } from 'vitest';
import { hasRawColour } from '../../scripts/raw-colour';

async function assertTokenStyles(directory: string): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await assertTokenStyles(path);
    else if (entry.name.endsWith('.module.css'))
      expect(hasRawColour(await readFile(path, 'utf8')), path).toBe(false);
  }
}
it('component CSS uses colour tokens exclusively', async () => {
  await assertTokenStyles('packages/ui/src/components');
  await assertTokenStyles('apps/desktop/src');
});
it('rejects hard-coded hex and functional colour notation', () => {
  for (const colour of [
    '#ff0000',
    '#FFF',
    'rgb(255, 0, 0)',
    'rgba(0,0,0,.5)',
    'hsl(0 100% 50%)',
  ])
    expect(hasRawColour(`.control { color: ${colour}; }`)).toBe(true);
  expect(
    hasRawColour(
      '.control { color: var(--or-text); background: currentColor; }',
    ),
  ).toBe(false);
});

const overlayStyles = new Set([
  'packages/ui/src/components/Palette.module.css',
  'packages/ui/src/components/Sheet.module.css',
  'apps/desktop/src/features/connections/Tooltip.module.css',
]);
async function assertCompositing(directory: string): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) await assertCompositing(path);
    else if (entry.name.endsWith('.css')) {
      const css = await readFile(path, 'utf8');
      if (!overlayStyles.has(path))
        expect(css, path).not.toMatch(/backdrop-filter\s*:/);
      expect(css, path).not.toMatch(/will-change\s*:|translateZ\s*\(/);
    }
  }
}
it('limits backdrop filters to mounted overlay styles and forbids permanent layer hints', async () => {
  await assertCompositing('packages/ui/src');
  await assertCompositing('apps/desktop/src');
});
