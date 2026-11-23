// SPDX-License-Identifier: Apache-2.0
import { fireEvent, screen } from '@testing-library/react';

/** Expand the sidebar's connection list when the pop-up has folded it. */
export function showConnections(): void {
  const popup = screen.queryByRole('button', { name: /^Connections:/ });
  if (popup?.getAttribute('aria-expanded') === 'false') fireEvent.click(popup);
}

/** Select a rail connection (stubbed IPC) and wait for its object tree. */
export async function connectTo(name = 'orders-local'): Promise<void> {
  showConnections();
  fireEvent.click(
    await screen.findByRole('button', { name: `Switch to ${name}` }),
  );
  await screen.findByRole('button', { name: /^Connections: .* · orders · / });
  await screen.findByRole('button', { name: /^Tables/ });
}
