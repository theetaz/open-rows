// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';

it('all interactive component selectors have a visible focus ring from tokens', () => {
  const controls = readFileSync(
    'packages/ui/src/components/Controls.module.css',
    'utf8',
  );
  const field = readFileSync(
    'packages/ui/src/components/TextField.module.css',
    'utf8',
  );
  for (const selector of [
    '.button:focus-visible',
    '.segment:focus-visible',
    '.switch:focus-visible',
  ])
    expect(controls).toContain(selector);
  expect(field).toContain('.frame:focus-within');
  for (const css of [controls, field])
    expect(css).toContain(
      'outline: var(--or-focus-width) solid var(--or-focus)',
    );
});
