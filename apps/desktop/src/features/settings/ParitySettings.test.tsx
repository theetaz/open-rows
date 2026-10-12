// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { parseEditorSettings } from '../query/editorSettings';
import { copySettings } from '../../results/copy/copySettings';
import { tableSettings } from './tableSettings';
import { applySettings, collectSettings, resetSettings } from './settingsFile';
import { parsePortableSettings } from './settingsValidation';
import { EditorPane } from './EditorPane';
import { TablesPane } from './TablesPane';

function open(
  section: 'editor' | 'tables',
): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      {section === 'editor' ? <EditorPane /> : <TablesPane />}
    </ThemeProvider>,
  );
  return userEvent.setup();
}

it('persists editor indentation and completion controls', async () => {
  const user = open('editor');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Indent with' }),
    '"tabs"',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Accept suggestion with' }),
    '"both"',
  );
  await user.click(
    screen.getByRole('switch', { name: 'Schema-qualified names' }),
  );
  await user.click(
    screen.getByRole('switch', { name: 'Close brackets and quotes' }),
  );
  expect(
    parseEditorSettings(localStorage.getItem('openrows.editor')),
  ).toMatchObject({
    indentWith: 'tabs',
    acceptKey: 'both',
    qualifyNames: true,
    closeDelimiters: false,
  });
});

it('persists striping, scrollbars and row count for tables', async () => {
  const user = open('tables');
  await user.click(
    screen.getByRole('switch', { name: 'Alternate row backgrounds' }),
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Grid scrollbars' }),
    'always',
  );
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'Row count' }),
    'exact',
  );
  expect(tableSettings.get()).toEqual({
    striped: false,
    scrollbars: 'always',
    rowCount: 'exact',
  });
});

it('rejects a comma decimal separator with comma-separated copies', async () => {
  const user = open('tables');
  const decimal = screen.getByRole('combobox', {
    name: 'Copy decimal separator',
  });
  await user.selectOptions(decimal, 'comma');
  expect(screen.getByRole('alert')).toHaveTextContent('cannot both be a comma');
  expect(copySettings.get().decimal).toBe('point');
  await user.selectOptions(
    screen.getByRole('combobox', { name: 'CSV delimiter' }),
    'semicolon',
  );
  await user.selectOptions(decimal, 'comma');
  expect(screen.queryByRole('alert')).toBeNull();
  expect(copySettings.get()).toMatchObject({
    delimiter: 'semicolon',
    decimal: 'comma',
  });
});

it('round-trips and resets table settings and rejects ambiguous portable copies', () => {
  applySettings({
    tables: { striped: false, scrollbars: 'always', rowCount: 'exact' },
  });
  expect(collectSettings().tables).toEqual({
    striped: false,
    scrollbars: 'always',
    rowCount: 'exact',
  });
  resetSettings('tables');
  expect(collectSettings().tables).toEqual({
    striped: true,
    scrollbars: 'quiet',
    rowCount: 'estimate',
  });
  expect(() =>
    parsePortableSettings(
      JSON.stringify({
        openrows: 1,
        copy: { delimiter: 'comma', decimal: 'comma' },
      }),
    ),
  ).toThrow('cannot both be a comma');
});
