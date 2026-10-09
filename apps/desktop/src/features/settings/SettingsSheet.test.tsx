// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider, useTheme } from '@openrows/ui';
import { SettingsSheet } from './SettingsSheet';
function State(): React.JSX.Element {
  const { appearance } = useTheme();
  return <output data-testid="state">{JSON.stringify(appearance)}</output>;
}
function setup(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <State />
      <SettingsSheet initial="appearance" onClose={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}
it('applies independent theme halves, fonts and live interface controls', async () => {
  const user = setup();
  await user.click(screen.getByRole('button', { name: 'Ocean light' }));
  await user.click(screen.getByRole('button', { name: 'Iris dark' }));
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Interface font' }),
    'arial',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Monospace font size' }),
    '16',
  );
  fireEvent.change(screen.getByRole('slider', { name: 'Glass opacity' }), {
    target: { value: '70' },
  });
  fireEvent.change(screen.getByRole('slider', { name: 'Panel animations' }), {
    target: { value: '200' },
  });
  expect(screen.getByTestId('state')).toHaveTextContent('"lightTheme":"ocean"');
  expect(screen.getByTestId('state')).toHaveTextContent('"darkTheme":"iris"');
  expect(screen.getByTestId('state')).toHaveTextContent('"monoSize":16');
  expect(screen.getByTestId('state')).toHaveTextContent('"opacity":70');
  expect(
    document.querySelector('[data-openrows-appearance]')?.textContent,
  ).toContain('--or-font-ui:Arial');
  await user.click(screen.getByRole('button', { name: 'Reset glass opacity' }));
  expect(screen.getByRole('slider', { name: 'Glass opacity' })).toHaveValue(
    '95',
  );
});
it('creates, exports, imports, edits and removes custom themes, rejects invalid JSON', async () => {
  const user = setup();
  await user.click(screen.getByRole('button', { name: 'Create theme' }));
  const editor = screen.getByRole('dialog', { name: 'Customize theme' });
  await user.clear(within(editor).getByLabelText('Theme name'));
  await user.type(within(editor).getByLabelText('Theme name'), 'My ocean');
  await user.click(within(editor).getByRole('button', { name: 'Save theme' }));
  const card = screen.getByText('My ocean').closest('article');
  if (!card) throw new Error('Custom card missing');
  await user.click(within(card).getByRole('button', { name: 'Export' }));
  const json = screen.getByLabelText<HTMLTextAreaElement>('Theme JSON').value;
  expect(JSON.parse(json)).toMatchObject({ name: 'My ocean' });
  await user.click(
    within(screen.getByRole('dialog', { name: 'Export theme' })).getByRole(
      'button',
      { name: 'Done' },
    ),
  );
  await user.click(screen.getByRole('button', { name: 'Add theme' }));
  fireEvent.change(screen.getByLabelText('Theme JSON'), {
    target: { value: '{}' },
  });
  await user.click(screen.getByRole('button', { name: 'Import theme' }));
  expect(screen.getByRole('alert')).toHaveTextContent('OpenRows theme');
  fireEvent.change(screen.getByLabelText('Theme JSON'), {
    target: { value: json.replace('My ocean', 'Imported ocean') },
  });
  await user.click(screen.getByRole('button', { name: 'Import theme' }));
  expect(
    screen.getByRole('button', { name: 'Imported ocean dark' }),
  ).toHaveAttribute('aria-pressed', 'true');
  await user.click(
    screen.getByRole('button', { name: 'Remove Imported ocean' }),
  );
  expect(screen.queryByText('Imported ocean')).toBeNull();
  await user.click(within(card).getByRole('button', { name: 'Edit' }));
  fireEvent.change(screen.getByLabelText('dark canvas'), {
    target: { value: '#ffffff' },
  });
  expect(screen.getByRole('button', { name: 'Save theme' })).toBeDisabled();
  await user.keyboard('{Escape}');
  expect(screen.getByRole('dialog', { name: 'Settings' })).toBeVisible();
  expect(screen.getByRole('button', { name: 'My ocean dark' })).toBeVisible();
});
it('restores defaults while keeping custom themes', async () => {
  const user = setup();
  await user.click(screen.getByRole('button', { name: 'Duplicate Forest' }));
  await user.click(screen.getByRole('button', { name: 'Save theme' }));
  await user.click(screen.getByRole('button', { name: 'Dark' }));
  await user.click(screen.getByRole('button', { name: 'Reset appearance' }));
  await user.click(screen.getByRole('button', { name: 'Restore defaults' }));
  expect(
    screen.getByRole('button', { name: 'Forest copy dark' }),
  ).toHaveAttribute('aria-pressed', 'false');
  expect(
    screen.getByRole('button', { name: 'OpenRows light' }),
  ).toHaveAttribute('aria-pressed', 'true');
  expect(document.documentElement).toHaveAttribute('data-theme', 'system');
});
it('switches General and Editor settings', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <SettingsSheet onClose={vi.fn()} />
    </ThemeProvider>,
  );
  const restore = screen.getByRole('switch', {
    name: 'Restore session on launch',
  });
  expect(restore).toHaveAttribute('aria-checked', 'true');
  await user.click(restore);
  await user.click(
    within(screen.getByRole('radiogroup', { name: 'Density' })).getByRole(
      'radio',
      { name: 'Compact' },
    ),
  );
  await user.click(
    within(screen.getByRole('radiogroup', { name: 'Grid rows' })).getByRole(
      'radio',
      { name: 'Comfortable' },
    ),
  );
  await user.click(screen.getByRole('switch', { name: 'Reduce motion' }));
  await user.click(screen.getByRole('radio', { name: '1 min' }));
  expect(JSON.parse(localStorage.getItem('openrows.general') ?? '{}')).toEqual({
    restoreSession: false,
    density: 'compact',
    gridDensity: 'comfortable',
    reduceMotion: true,
    keepaliveSeconds: 60,
  });
  await user.click(screen.getByRole('radio', { name: 'Editor' }));
  await user.click(screen.getByRole('switch', { name: 'Wrap lines' }));
  expect(localStorage.getItem('openrows.editor')).toContain('"wrap":true');
});
it('records shortcuts, reports conflicts and replaces on request', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <SettingsSheet initial="shortcuts" onClose={vi.fn()} />
    </ThemeProvider>,
  );
  const save = screen.getByRole('button', {
    name: 'Shortcut for Save SQL file',
  });
  expect(save).toHaveTextContent('⌘S');
  await user.click(save);
  expect(save).toHaveTextContent('Type shortcut…');
  await user.keyboard('{Meta>}j{/Meta}');
  expect(save).toHaveTextContent('⌘J');
  await user.click(save);
  await user.keyboard('{Meta>}q{/Meta}');
  expect(screen.getByRole('alert')).toHaveTextContent('reserved for Quit');
  await user.click(save);
  await user.keyboard('{Meta>}t{/Meta}');
  expect(screen.getByRole('alert')).toHaveTextContent('“New query tab”');
  await user.click(screen.getByRole('button', { name: 'Replace' }));
  expect(save).toHaveTextContent('⌘T');
  expect(
    screen.getByRole('button', { name: 'Shortcut for New query tab' }),
  ).toHaveTextContent('—');
  await user.click(screen.getByRole('button', { name: 'Restore defaults' }));
  expect(save).toHaveTextContent('⌘S');
});
it('searches every pane and lists only matching settings', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <SettingsSheet initial="general" onClose={vi.fn()} />
    </ThemeProvider>,
  );
  const search = screen.getByRole('searchbox', { name: 'Search settings' });
  await user.type(search, 'font');
  expect(
    screen.getByRole('heading', { name: 'Settings / Search' }),
  ).toBeVisible();
  expect(
    screen.getByRole('combobox', { name: 'Interface font' }),
  ).toBeVisible();
  expect(
    screen.getByRole('region', { name: 'Editor settings' }),
  ).toBeInTheDocument();
  expect(screen.queryByRole('switch', { name: 'Wrap lines' })).toBeNull();
  expect(
    document.querySelector('[aria-label="General settings"]'),
  ).toHaveAttribute('hidden');
  await user.clear(search);
  await user.type(search, 'save sql');
  expect(
    screen.getByRole('button', { name: 'Shortcut for Save SQL file' }),
  ).toBeVisible();
  expect(
    screen.queryByRole('button', { name: 'Shortcut for New query tab' }),
  ).toBeNull();
  await user.clear(search);
  await user.type(search, 'zzz');
  expect(screen.getByRole('status')).toHaveTextContent(
    'No settings match “zzz”.',
  );
  await user.click(screen.getByRole('radio', { name: 'Editor' }));
  expect(search).toHaveValue('');
  expect(screen.getByRole('switch', { name: 'Wrap lines' })).toBeVisible();
});
