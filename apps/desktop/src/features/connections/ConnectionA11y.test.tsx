// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import axe from 'axe-core';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { profile, sampleProfiles, server } from '../../test/ipcStub';
import { ConnectionRail } from './ConnectionRail';
import { ConnectionSheet } from './ConnectionSheet';
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
          onSettings={vi.fn()}
        />
      </main>
    </ThemeProvider>,
  );
  expect(screen.getByRole('searchbox')).toBeInTheDocument();
  expect(await violations(container)).toEqual([]);
});

it.each([
  ['a new connection', undefined],
  ['a saved password', profile('orders', { hasPassword: true })],
  ['ask every time', profile('vault', { passwordStorage: 'ask' })],
])('the connection sheet for %s has zero axe violations', async (_, saved) => {
  const { container } = render(
    <ThemeProvider>
      <ConnectionSheet
        profile={saved}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  expect(await violations(container)).toEqual([]);
});

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
