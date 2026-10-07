// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider, useTheme } from '@openrows/ui';
import { AppearanceSettings } from './AppearanceSettings';
function State(): React.JSX.Element {
  const { appearance } = useTheme();
  return <output data-testid="state">{JSON.stringify(appearance)}</output>;
}
function setup(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <State />
      <AppearanceSettings onClose={vi.fn()} />
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
  expect(screen.getByRole('dialog', { name: 'Appearance' })).toBeVisible();
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
