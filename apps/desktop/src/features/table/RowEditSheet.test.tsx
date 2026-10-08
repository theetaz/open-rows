// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { RowEditSheet } from './RowEditSheet';
import type { RowEditing } from './useRowEditing';

it.each(['precise value', null])(
  'focuses the clicked field or its NULL control: %s',
  (note) => {
    const editing = {
      review: null,
      draft: {
        id: 'synthetic',
        number: 1,
        kind: 'update',
        original: { id: '7', note },
        values: {},
        focusField: 'note',
      },
      info: {
        key: ['id'],
        protected: [],
        readOnly: false,
        confirmWrites: false,
        production: false,
        columns: [
          {
            name: 'id',
            dataType: 'integer',
            nullable: false,
            default: null,
            comment: null,
            primaryKey: true,
            enumLabels: null,
          },
          {
            name: 'note',
            dataType: 'text',
            nullable: true,
            default: null,
            comment: null,
            primaryKey: false,
            enumLabels: null,
          },
        ],
      },
      close: vi.fn(),
      change: vi.fn(),
    } as unknown as RowEditing;
    render(<RowEditSheet editing={editing} />);
    const input = screen.getByRole('textbox', { name: 'Edit note' });
    if (note === null) {
      expect(input).toBeDisabled();
      expect(screen.getByRole('checkbox', { name: 'NULL note' })).toHaveFocus();
    } else {
      expect(input).toHaveValue(note);
      expect(input).toHaveFocus();
    }
    expect(screen.getByRole('textbox', { name: 'Edit id' })).not.toHaveFocus();
  },
);
