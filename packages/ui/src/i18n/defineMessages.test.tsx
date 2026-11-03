// SPDX-License-Identifier: Apache-2.0
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defineMessages } from './defineMessages';

const { t, tn, rich, richn } = defineMessages({
  saved: 'Saved',
  greeting: 'Hello {name}, you have {count} tabs',
  repeated: '{name} and {name}',
  rows: { one: '{count} row', other: '{count} rows' },
  rowsIn: { one: '{count} row in {table}', other: '{count} rows in {table}' },
  deleted: 'Deleted {name}.',
  deletedRows: {
    one: 'Deleted {count} row from {table}.',
    other: 'Deleted {count} rows from {table}.',
  },
});

describe('t', () => {
  it('returns plain messages unchanged', () => {
    expect(t('saved')).toBe('Saved');
  });

  it('fills every placeholder, in any order and more than once', () => {
    expect(t('greeting', { count: 3, name: 'Ada' })).toBe(
      'Hello Ada, you have 3 tabs',
    );
    expect(t('repeated', { name: 'x' })).toBe('x and x');
  });

  it('never expands a value a second time', () => {
    expect(t('repeated', { name: '{name}' })).toBe('{name} and {name}');
    expect(t('greeting', { name: '{count}', count: 1 })).toBe(
      'Hello {count}, you have 1 tabs',
    );
  });

  it('rejects unknown keys and missing values at compile time', () => {
    // @ts-expect-error unknown key
    expect(() => t('missing')).not.toThrow();
    // @ts-expect-error missing placeholder value
    expect(t('deleted')).toBe('Deleted {name}.');
    // @ts-expect-error a plural message needs tn
    expect(t('rows')).toEqual({ one: '{count} row', other: '{count} rows' });
    // @ts-expect-error a plain message cannot take a count
    expect(() => tn('saved', 1)).toThrow();
  });
});

describe('tn', () => {
  it('chooses the form with Intl.PluralRules', () => {
    expect(tn('rows', 0)).toBe('0 rows');
    expect(tn('rows', 1)).toBe('1 row');
    expect(tn('rows', 2)).toBe('2 rows');
  });

  it('keeps a formatted count and other values', () => {
    expect(tn('rows', 2500, { count: '2,500' })).toBe('2,500 rows');
    expect(tn('rowsIn', 1, { table: 'orders' })).toBe('1 row in orders');
  });
});

describe('rich', () => {
  it('renders elements in place of placeholders', () => {
    const { container } = render(
      <p>{rich('deleted', { name: <strong>orders</strong> })}</p>,
    );
    expect(container.innerHTML).toBe('<p>Deleted <strong>orders</strong>.</p>');
  });

  it('renders nothing for an empty value, like JSX', () => {
    const { container } = render(<p>{rich('deleted', { name: null })}</p>);
    expect(container.textContent).toBe('Deleted .');
  });

  it('chooses plural forms for rich messages', () => {
    const { container } = render(
      <p>{richn('deletedRows', 2, { table: <code>orders</code> })}</p>,
    );
    expect(container.innerHTML).toBe(
      '<p>Deleted 2 rows from <code>orders</code>.</p>',
    );
  });
});
