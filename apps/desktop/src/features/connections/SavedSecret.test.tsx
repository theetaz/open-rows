// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { revealSecret } from '@openrows/ipc';
import { ThemeProvider } from '@openrows/ui';
import { profile } from '../../test/ipcStub';
import { ConnectionSheet } from './ConnectionSheet';
import { REVEAL_TIMEOUT_MS } from './useRevealedSecret';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const SECRET = 'pw-revealed-synthetic';

afterEach(() => {
  vi.useRealTimers();
});

function mount() {
  const existing = profile('orders', { hasPassword: true });
  return render(
    <ThemeProvider>
      <ConnectionSheet
        profile={existing}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        onDelete={vi.fn()}
        onDuplicate={vi.fn()}
      />
    </ThemeProvider>,
  );
}

it('reads a saved password only when Reveal is clicked and hides it again on blur', async () => {
  const user = userEvent.setup();
  mount();
  const field = screen.getByLabelText('Password');
  expect(field).toHaveAttribute('type', 'password');
  expect(field).not.toHaveValue(SECRET);
  expect(revealSecret).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Reveal password' }));
  expect(revealSecret).toHaveBeenCalledWith('id-orders', 'password');
  expect(await screen.findByDisplayValue(SECRET)).toHaveAttribute(
    'type',
    'text',
  );
  expect(field).toHaveFocus();
  expect(screen.getByRole('button', { name: 'Hide password' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await user.click(screen.getByRole('textbox', { name: 'Database' }));
  expect(field).toHaveAttribute('type', 'password');
  expect(screen.queryByDisplayValue(SECRET)).toBeNull();
});

it('hides a revealed password after the timeout and forgets it when the sheet closes', async () => {
  vi.useFakeTimers();
  const view = mount();
  fireEvent.click(screen.getByRole('button', { name: 'Reveal password' }));
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByDisplayValue(SECRET)).toBeInTheDocument();
  act(() => {
    vi.advanceTimersByTime(REVEAL_TIMEOUT_MS);
  });
  expect(screen.queryByDisplayValue(SECRET)).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: 'Reveal password' }));
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByDisplayValue(SECRET)).toBeInTheDocument();
  view.unmount();
  expect(document.body.innerHTML).not.toContain(SECRET);
});

it('reports a Keychain read that fails and stays masked', async () => {
  const user = userEvent.setup();
  vi.mocked(revealSecret).mockRejectedValueOnce(
    new Error('The Keychain denied access'),
  );
  mount();
  await user.click(screen.getByRole('button', { name: 'Reveal password' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'The Keychain denied access',
  );
  expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password');
});
