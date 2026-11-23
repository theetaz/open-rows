// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import axe from 'axe-core';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles, server } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';
import { mountFlow, openStep } from './flow/flowTestKit';
import { PasswordPromptSheet } from './PasswordPromptSheet';
import type { SessionStatus } from './sessions';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

/** jsdom has no layout, so contrast is checked by the token contrast report instead. */
async function violations(container: HTMLElement): Promise<axe.Result[]> {
  const result = await axe.run(container, {
    rules: { 'color-contrast': { enabled: false } },
  });
  return result.violations;
}

const connected: SessionStatus = {
  state: 'connected',
  insecure: [],
  server: server('17.2'),
  databases: [{ name: 'orders', state: 'open', sessionId: 's1' }],
  generation: 1,
};

it('the connection rail with its search has zero axe violations', async () => {
  const { container } = render(
    <ThemeProvider>
      <main>
        <ConnectionRail
          profiles={sampleProfiles}
          activeId="id-orders-local"
          statusOf={(id) =>
            id === 'id-orders-local' ? connected : { state: 'disconnected' }
          }
          onSelect={vi.fn()}
          onNew={vi.fn()}
        />
      </main>
    </ThemeProvider>,
  );
  expect(screen.getByRole('searchbox')).toBeInTheDocument();
  expect(await violations(container)).toEqual([]);
});

it.each([
  ['the engine step', {}],
  ['new server details', { engine: 'postgres' as const }],
  ['new file details', { engine: 'duckdb' as const }],
  ['a saved password', { profile: profile('orders', { hasPassword: true }) }],
  ['ask every time', { profile: profile('vault', { passwordStorage: 'ask' }) }],
])('the connection flow for %s has zero axe violations', async (_, options) => {
  await mountFlow(options);
  expect(await violations(document.body)).toEqual([]);
});

it.each(['Security', 'Options', 'Test and save'])(
  'the connection flow step %s has zero axe violations',
  async (step) => {
    const { user } = await mountFlow({ profile: profile('orders') });
    await openStep(user, step);
    expect(await violations(document.body)).toEqual([]);
  },
);

it('the password prompt sheet has zero axe violations', async () => {
  const { container } = render(
    <ThemeProvider>
      <PasswordPromptSheet
        request={{
          profile: profile('vault', { passwordStorage: 'ask' }),
          submit: vi.fn(),
          cancel: vi.fn(),
        }}
      />
    </ThemeProvider>,
  );
  expect(await violations(container)).toEqual([]);
});
