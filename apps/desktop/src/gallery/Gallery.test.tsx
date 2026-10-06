// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import axe from 'axe-core';
import { ThemeProvider } from '@openrows/ui';
import { Gallery } from './Gallery';

vi.mock('@openrows/ipc', () => ({
  getAppInfo: vi.fn().mockResolvedValue({
    name: 'OpenRows',
    version: '0.1.0',
    buildProfile: 'debug',
  }),
}));
it.each(['light', 'dark', 'system'])(
  'gallery has zero axe violations in %s',
  async (theme) => {
    localStorage.setItem('openrows.theme', theme);
    const { container } = render(
      <ThemeProvider>
        <main>
          <Gallery />
        </main>
      </ThemeProvider>,
    );
    await screen.findByText('OpenRows 0.1.0 · debug');
    const result = await axe.run(container, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(result.violations).toEqual([]);
  },
);
it('renders all sections, swatches, states and a functional theme switcher', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Gallery />
    </ThemeProvider>,
  );
  for (const heading of [
    'Controls',
    'Fields & selection',
    'Environment & status',
    'Type & colour tokens',
  ])
    expect(screen.getByRole('heading', { name: heading })).toBeInTheDocument();
  for (const name of [
    'local',
    'dev',
    'staging',
    'prod',
    'Pending',
    'Shipped',
    'Paid',
    'Cancelled',
    'Failed',
  ])
    expect(screen.getAllByText(name).length).toBeGreaterThan(0);
  expect(screen.getByText('control-fill')).toBeInTheDocument();
  expect(screen.getByText('Enter a valid connection URL.')).toBeInTheDocument();
  await user.click(screen.getByRole('radio', { name: 'Dark' }));
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  expect(localStorage.getItem('openrows.theme')).toBe('dark');
});
