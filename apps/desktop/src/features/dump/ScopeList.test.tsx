// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it } from 'vitest';
import { ScopeList } from './ScopeList';

it('bounds catalog rendering and exposes later objects without truncating selection', async () => {
  const user = userEvent.setup();
  render(
    <ScopeList
      rows={Array.from({ length: 10_000 }, (_, id) => id)}
      render={(id) => (
        <input key={id} type="checkbox" aria-label={`Object ${String(id)}`} />
      )}
    />,
  );
  expect(screen.getAllByRole('checkbox')).toHaveLength(80);
  expect(screen.queryByLabelText('Object 80')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Next objects' }));
  expect(screen.getByLabelText('Object 80')).toBeInTheDocument();
  expect(screen.getAllByRole('checkbox')).toHaveLength(80);
  expect(screen.getByRole('status')).toHaveTextContent('Page 2 of 125');
});
