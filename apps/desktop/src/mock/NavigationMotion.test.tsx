// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ConnectionRail } from '../features/connections';
import { profile } from '../test/ipcStub';
import { Workspace } from './Workspace';
import { ThemeProvider } from '@openrows/ui';
import { geometry, material } from '@openrows/ui/tokens';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);
afterEach(() => {
  vi.useRealTimers();
});
it('rail indicator responds to reduced motion without database disclosures', () => {
  const props = {
    profiles: [profile('orders'), profile('events')],
    activeId: 'id-orders',
    statusOf: () => ({ state: 'disconnected' as const }),
    onSelect: vi.fn(),
    onNew: vi.fn(),
    onGallery: vi.fn(),
  };
  const { rerender } = render(<ConnectionRail {...props} />);
  const item = screen
    .getByRole('button', { name: 'Switch to orders' })
    .querySelector('[data-state]');
  expect(item).toHaveAttribute('data-state', 'selected');
  expect(item).toHaveAttribute('data-motion', 'full');
  rerender(<ConnectionRail {...props} activeId="id-events" />);
  expect(item).toHaveAttribute('data-state', 'idle');
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
    'apps/desktop/src/features/connections/Rail.module.css',
    'utf8',
  );
  const palette = readFileSync(
    'packages/ui/src/components/Palette.module.css',
    'utf8',
  );
  expect(material['motion-disclosure']).toBe('220ms');
  expect(material['motion-popover']).toBe('160ms');
  expect(material['motion-easing']).toBe('cubic-bezier(0.32, 0.72, 0, 1)');
  expect(rail).toContain(
    'height var(--or-motion-popover) var(--or-motion-easing)',
  );
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
it('system database selection produces an empty tab set and rail state without opening a connection', async () => {
  vi.useFakeTimers();
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  await act(async () => {
    fireEvent.keyDown(window, { key: 'k', metaKey: true });
    await import('./WorkspacePalettes');
  });
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
