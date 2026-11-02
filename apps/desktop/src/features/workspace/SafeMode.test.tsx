// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import type { ProfileDto } from '@openrows/ipc';
import { executeQuery, listProfiles, updateProfile } from '@openrows/ipc';
import { profile } from '../../test/ipcStub';
import { connectTo } from '../../test/connect';
import { safeModeState } from '../security/policies';
import { Workspace } from './Workspace';
// Load the real lazy surfaces during collection, outside the test hook deadline.
import './WorkspacePalettes';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../query/QueryEditor', async () => ({
  QueryEditor: (await import('../query/testEditor')).TextareaEditor,
}));

const READ_ONLY = 'This connection is read-only: nothing was run.';

function mount(...profiles: ProfileDto[]): ReturnType<typeof userEvent.setup> {
  vi.mocked(listProfiles).mockResolvedValue({ profiles, unreadable: [] });
  vi.stubGlobal('innerWidth', 1280);
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup({ skipHover: true });
}

async function palette(command: string): Promise<void> {
  await act(async () => {
    fireEvent.keyDown(window, { key: 'p', metaKey: true, shiftKey: true });
    await import('./WorkspacePalettes');
  });
  const input = within(await screen.findByRole('dialog')).getByRole('combobox');
  fireEvent.change(input, { target: { value: command } });
  fireEvent.keyDown(input, { key: 'Enter' });
  await waitFor(() => {
    expect(screen.queryByRole('combobox', { name: /command/i })).toBeNull();
  });
}

beforeEach(() => {
  vi.mocked(executeQuery).mockReset();
  vi.mocked(updateProfile).mockReset();
});

it('reports a read-only non-prod connection as protected and clicking loosens nothing', async () => {
  const user = mount(profile('orders-local', { safety: 'readOnly' }));
  await connectTo();
  const safe = screen.getByRole('button', { name: 'Safe mode: Read-only' });
  expect(safe).toHaveAttribute('data-on', 'true');
  expect(safe).not.toHaveAttribute('aria-pressed');
  await user.click(safe);
  const sheet = await screen.findByRole('dialog', {
    name: 'Edit orders-local',
  });
  const policy = within(sheet).getByRole('combobox', { name: 'Safety policy' });
  expect(policy).toHaveValue('readOnly');
  expect(policy).toHaveFocus();
  await user.click(within(sheet).getByRole('button', { name: 'Cancel' }));
  expect(updateProfile).not.toHaveBeenCalled();
  expect(
    screen.getByRole('button', { name: 'Safe mode: Read-only' }),
  ).toHaveAttribute('data-on', 'true');
});

it('changes the state only when the saved policy changes', async () => {
  const saved = profile('orders-local', { safety: 'unrestricted' });
  const user = mount(saved);
  await connectTo();
  await user.click(
    screen.getByRole('button', { name: 'Safe mode: Unrestricted' }),
  );
  const sheet = await screen.findByRole('dialog', {
    name: 'Edit orders-local',
  });
  vi.mocked(updateProfile).mockImplementation((_id, input) => {
    const updated = { ...saved, safety: input.safety };
    vi.mocked(listProfiles).mockResolvedValue({
      profiles: [updated],
      unreadable: [],
    });
    return Promise.resolve(updated);
  });
  await user.selectOptions(
    within(sheet).getByRole('combobox', { name: 'Safety policy' }),
    'confirmWrites',
  );
  await user.click(within(sheet).getByRole('button', { name: 'Save' }));
  await screen.findByRole('button', { name: 'Safe mode: Confirm writes' });
  expect(vi.mocked(updateProfile).mock.calls[0]?.[1]).toMatchObject({
    safety: 'confirmWrites',
  });
});

it('tells the truth about an unrestricted production connection', async () => {
  mount(
    profile('analytics-prod', {
      env: { kind: 'prod', label: null, color: '#dc2626' },
      safety: 'unrestricted',
    }),
  );
  await connectTo('analytics-prod');
  expect(
    screen.getByRole('button', { name: 'Safe mode: Unrestricted' }),
  ).toHaveAttribute('data-on', 'false');
});

it('counts a read-only database file as protected, whatever the policy says', () => {
  const file = {
    path: '/tmp/a.db',
    readOnly: true,
    create: false,
    attached: [],
  };
  expect(safeModeState(profile('a', { file }))).toMatchObject({
    on: true,
    label: 'Read-only file',
  });
  expect(
    safeModeState(profile('b', { file: { ...file, readOnly: false } })),
  ).toMatchObject({ on: false, label: 'Unrestricted' });
});

it('the palette opens the setting instead of switching protection', async () => {
  mount(profile('orders-local', { safety: 'readOnly' }));
  await connectTo();
  await palette('Safe mode settings');
  expect(
    await screen.findByRole('dialog', { name: 'Edit orders-local' }),
  ).toBeVisible();
  expect(updateProfile).not.toHaveBeenCalled();
});

it('a write run from the palette on a read-only connection goes through the guard and is blocked', async () => {
  vi.mocked(executeQuery).mockResolvedValue({
    kind: 'blocked',
    message: READ_ONLY,
    statements: [],
  });
  mount(profile('orders-local', { safety: 'readOnly' }));
  await connectTo();
  await palette('New query tab');
  const editor = await screen.findByRole('textbox', { name: /SQL/ });
  fireEvent.change(editor, { target: { value: 'DELETE FROM orders' } });
  await palette('Run current statement');
  expect(
    await screen.findByRole('list', { name: 'Messages' }),
  ).toHaveTextContent('read-only');
  expect(screen.getByText('Blocked: read-only connection')).toBeVisible();
  expect(executeQuery).toHaveBeenCalledTimes(1);
  expect(vi.mocked(executeQuery).mock.calls[0]?.[0]).toMatchObject({
    sql: 'DELETE FROM orders',
    confirmToken: null,
  });
  expect(screen.queryByRole('grid')).toBeNull();
});
