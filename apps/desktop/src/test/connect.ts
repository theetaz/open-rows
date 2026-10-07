// SPDX-License-Identifier: Apache-2.0
import { fireEvent, screen } from '@testing-library/react';

/** Select a rail connection (stubbed IPC) and wait for its object tree. */
export async function connectTo(name = 'orders-local'): Promise<void> {
  fireEvent.click(
    await screen.findByRole('button', { name: `Switch to ${name}` }),
  );
  await screen.findByRole('button', { name: 'Opened databases (orders)' });
  await screen.findByRole('button', { name: /^Tables/ });
}
