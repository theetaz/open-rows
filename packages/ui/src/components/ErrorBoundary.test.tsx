// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ErrorBoundary } from './ErrorBoundary';

function Broken(): React.JSX.Element {
  throw new Error('stale mock');
}

it('shows a reload pane instead of a blank window when a child throws', async () => {
  const quiet = vi.spyOn(console, 'error').mockImplementation(() => undefined);
  const onError = vi.fn();
  const onReload = vi.fn();
  render(
    <ErrorBoundary onError={onError} onReload={onReload}>
      <Broken />
    </ErrorBoundary>,
  );
  expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong');
  expect(onError).toHaveBeenCalledOnce();
  await userEvent.setup().click(screen.getByRole('button', { name: 'Reload' }));
  expect(onReload).toHaveBeenCalledOnce();
  quiet.mockRestore();
});

it('renders its children while nothing fails', () => {
  render(
    <ErrorBoundary>
      <p>Workspace</p>
    </ErrorBoundary>,
  );
  expect(screen.getByText('Workspace')).toBeVisible();
  expect(screen.queryByRole('alert')).toBeNull();
});
