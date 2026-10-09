// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { commandArguments, commandErrors } from './credentialForm';
import { emptyDraft, toInput } from './form';
it('preserves shell characters as literal argv and validates bounds', () => {
  expect(commandArguments('["; $(touch x)", "", "a b"]')).toEqual([
    '; $(touch x)',
    '',
    'a b',
  ]);
  for (const text of ['[1]', '"cmd"', '['])
    expect(commandArguments(text)).toBeNull();
  const draft = {
    ...emptyDraft(),
    passwordStorage: 'command' as const,
    commandProgram: '/usr/bin/provider',
    commandTimeout: '10',
  };
  expect(commandErrors(draft)).toEqual({});
  for (const value of ['0', '121', '1.5'])
    expect(commandErrors({ ...draft, commandTimeout: value })).toHaveProperty(
      'commandTimeout',
    );
  expect(
    commandErrors({ ...draft, commandProgram: 'provider' }),
  ).toHaveProperty('commandProgram');
});
it('none and command policies never submit a typed database password', () => {
  for (const passwordStorage of ['none', 'command'] as const) {
    const draft = {
      ...emptyDraft(),
      passwordStorage,
      passwordMode: 'set' as const,
      password: 'pw',
      commandProgram: '/usr/bin/provider',
    };
    expect(toInput(draft).password).toEqual({ kind: 'clear' });
  }
});
