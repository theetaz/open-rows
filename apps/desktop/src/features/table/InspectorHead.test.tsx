// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { InspectorHead } from './InspectorHead';
import type { RowField } from './row';

const fields: RowField[] = [
  { index: 0, name: 'id', type: 'int4', text: '7', state: 'normal' },
  { index: 1, name: 'note', type: 'text', text: 'NULL', state: 'null' },
  { index: 2, name: 'tag', type: 'text', text: 'EMPTY', state: 'empty' },
  {
    index: 3,
    name: 'rowid',
    type: null,
    text: '1',
    state: 'normal',
    hidden: true,
  },
];

it('titles the row, copies it as JSON and hides the inspector', async () => {
  const user = userEvent.setup();
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  const hide = vi.fn();
  render(
    <InspectorHead
      title="Row 9"
      subtitle="notes"
      fields={fields}
      onHide={hide}
    />,
  );
  expect(screen.getByText('Row 9')).toBeInTheDocument();
  expect(screen.getByText('notes')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: 'Copy row as JSON' }));
  expect(write).toHaveBeenCalledWith(
    JSON.stringify({ id: '7', note: null, tag: '' }, null, 2),
  );
  await user.click(screen.getByRole('button', { name: 'Hide details' }));
  expect(hide).toHaveBeenCalled();
});

it('offers no copy or hide without fields or a hide action', () => {
  render(<InspectorHead title="Row 1" subtitle="id" />);
  expect(screen.queryByRole('button')).toBeNull();
});
