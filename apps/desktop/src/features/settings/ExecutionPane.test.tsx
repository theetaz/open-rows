// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { parseEditorSettings } from '../query/editorSettings';
import { executionSettings } from '../query/executionSettings';
import { SettingsSheet } from './SettingsSheet';

function open(
  section: 'editor' | 'execution',
): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <SettingsSheet initial={section} onClose={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}

const editor = (): ReturnType<typeof parseEditorSettings> =>
  parseEditorSettings(localStorage.getItem('openrows.editor'));

it('sets what ⌘↵ runs, the automatic LIMIT and confirmations', async () => {
  const user = open('execution');
  await user.selectOptions(
    screen.getByRole('combobox', { name: '⌘↵ runs' }),
    'all',
  );
  expect(editor().runDefault).toBe('all');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Limit SELECT results' }),
    '1000',
  );
  await user.click(screen.getByRole('switch', { name: 'Run selection' }));
  await user.click(
    screen.getByRole('switch', { name: 'Confirm multiple statements' }),
  );
  await user.click(screen.getByRole('switch', { name: 'Show server notices' }));
  expect(executionSettings.get()).toEqual({
    runSelection: false,
    autoLimit: 1000,
    confirmMultiple: true,
    showNotices: false,
  });
  await user.click(screen.getByRole('switch', { name: 'Return on error' }));
  expect(editor().returnOnError).toBe(false);
});

it('sets display and completion options of the SQL editor', async () => {
  const user = open('editor');
  await user.click(screen.getByRole('switch', { name: 'Line numbers' }));
  await user.click(screen.getByRole('switch', { name: 'Code folding' }));
  await user.click(screen.getByRole('switch', { name: 'Suggest columns' }));
  await user.click(screen.getByRole('switch', { name: 'Suggest snippets' }));
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Font' }),
    '"Menlo"',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Suggestion delay' }),
    '500',
  );
  expect(editor()).toMatchObject({
    lineNumbers: false,
    folding: false,
    completeColumns: false,
    completeSnippets: false,
    completeTables: true,
    fontFamily: 'Menlo',
    completionDelay: 500,
  });
});
