// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ConnectionRail } from './ConnectionRail';
import { Workspace } from './Workspace';
import { ThemeProvider } from '@openrows/ui';
import { geometry, material } from '@openrows/ui/tokens';
afterEach(() => {
  vi.useRealTimers();
});
it('rail opening/closing uses retained height/fade state, inertness and reduced-motion logic', () => {
  const props = {
    selected: 0,
    opened: ['orders'],
    database: 'orders',
    onSelect: vi.fn(),
    onGallery: vi.fn(),
    onDatabase: vi.fn(),
    onCloseDatabase: vi.fn(),
  };
  const { rerender } = render(<ConnectionRail {...props} />);
  const item = screen
    .getByRole('button', {
      name: 'Activate database orders_archive',
      hidden: true,
    })
    .closest('[data-open]');
  expect(item).toHaveAttribute('data-open', 'false');
  expect(item).toHaveAttribute('inert');
  rerender(<ConnectionRail {...props} opened={['orders', 'orders_archive']} />);
  expect(item).toHaveAttribute('data-open', 'true');
  expect(item).not.toHaveAttribute('inert');
  expect(item).toHaveAttribute('data-motion', 'full');
  rerender(<ConnectionRail {...props} />);
  expect(item).toHaveAttribute('data-open', 'false');
  expect(item).toHaveAttribute('inert');
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
  rerender(<ConnectionRail {...props} />);
  expect(item).toHaveAttribute('data-motion', 'reduced');
});
it('uses token durations/easing and removes rail/popover animations under reduced motion', () => {
  const rail = readFileSync(
    'apps/desktop/src/mock/Workspace.module.css',
    'utf8',
  );
  const palette = readFileSync(
    'packages/ui/src/components/Palette.module.css',
    'utf8',
  );
  expect(material['motion-disclosure']).toBe('220ms');
  expect(material['motion-popover']).toBe('160ms');
  expect(material['motion-easing']).toBe('cubic-bezier(0.32, 0.72, 0, 1)');
  expect(rail).toContain('grid-template-rows var(--or-motion-disclosure)');
  expect(rail).toContain('opacity var(--or-motion-disclosure)');
  for (const css of [rail, palette]) {
    expect(css).toContain('prefers-reduced-motion: reduce');
    expect(css).toContain("[data-motion='reduced']");
  }
  expect(palette).toContain('var(--or-motion-popover) var(--or-motion-easing)');
  expect(palette).toContain('transform: scale(0.96)');
});
it('native window buttons use the screenshot-verified inset for the 48 px toolbar', () => {
  const config = JSON.parse(
    readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'),
  ) as {
    app: {
      windows: {
        trafficLightPosition: { x: number; y: number };
        titleBarStyle: string;
        decorations: boolean;
      }[];
    };
  };
  const window = config.app.windows[0];
  expect(window?.trafficLightPosition).toEqual({ x: 16, y: 26 });
  expect(window?.titleBarStyle).toBe('Overlay');
  expect(window?.decorations).toBe(true);
  expect(geometry['toolbar-height']).toBe('48px');
  expect(geometry['traffic-light-clearance']).toBe('78px');
});
it('system database selection produces an empty tab set and rail state without opening a connection', () => {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  fireEvent.keyDown(window, { key: 'k', metaKey: true });
  fireEvent.click(
    screen.getByRole('checkbox', { name: 'Show system databases' }),
  );
  fireEvent.change(screen.getByRole('combobox', { name: 'Switch database' }), {
    target: { value: 'postgres' },
  });
  fireEvent.keyDown(screen.getByRole('combobox', { name: 'Switch database' }), {
    key: 'Enter',
  });
  act(() => {
    vi.advanceTimersByTime(160);
  });
  expect(screen.queryAllByRole('tab')).toHaveLength(0);
  expect(screen.getByText('No open tabs')).toBeVisible();
});
