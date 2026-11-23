// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { revealSecret } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { mountFlow } from './flow/flowTestKit';
import { REVEAL_TIMEOUT_MS } from './useRevealedSecret';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const SECRET = 'pw-revealed-synthetic';

afterEach(() => {
  vi.useRealTimers();
});

function mount() {
  return mountFlow({ profile: profile('orders', { hasPassword: true }) });
}

it('reads a saved password only when Reveal is clicked and hides it again on blur', async () => {
  const { user } = await mount();
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

it('hides a revealed password after the timeout and forgets it when the tab closes', async () => {
  const view = await mount();
  vi.useFakeTimers();
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
  vi.mocked(revealSecret).mockRejectedValueOnce(
    new Error('The Keychain denied access'),
  );
  const { user } = await mount();
  await user.click(screen.getByRole('button', { name: 'Reveal password' }));
  expect(await screen.findByRole('alert')).toHaveTextContent(
    'The Keychain denied access',
  );
  expect(screen.getByLabelText('Password')).toHaveAttribute('type', 'password');
});
