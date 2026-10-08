// SPDX-License-Identifier: Apache-2.0
import { readFileSync } from 'node:fs';
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ConnectionRail } from '../features/connections';
import { profile } from '../test/ipcStub';
import { geometry, material } from '@openrows/ui/tokens';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);
afterEach(() => {
  vi.useRealTimers();
});
it('workspace selection remains accessible under reduced motion', () => {
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
  const onSelect = vi.fn();
  const props = {
    profiles: [profile('orders'), profile('events')],
    activeId: 'id-orders',
    statusOf: () => ({ state: 'disconnected' as const }),
    onSelect,
    onNew: vi.fn(),
    onGallery: vi.fn(),
  };
  const { rerender } = render(<ConnectionRail {...props} />);
  const orders = screen.getByRole('button', { name: 'Switch to orders' });
  const events = screen.getByRole('button', { name: 'Switch to events' });
  expect(orders).toHaveAttribute('aria-pressed', 'true');
  orders.focus();
  fireEvent.keyDown(orders, { key: 'ArrowDown' });
  expect(events).toHaveFocus();
  fireEvent.click(events);
  expect(onSelect).toHaveBeenCalledWith('id-events');
  rerender(<ConnectionRail {...props} activeId="id-events" />);
  expect(orders).toHaveAttribute('aria-pressed', 'false');
  expect(events).toHaveAttribute('aria-pressed', 'true');
  vi.unstubAllGlobals();
});
it('uses token durations/easing and removes popover animations under reduced motion', () => {
  const palette = readFileSync(
    'packages/ui/src/components/Palette.module.css',
    'utf8',
  );
  expect(material['motion-disclosure']).toBe('220ms');
  expect(material['motion-popover']).toBe('160ms');
  expect(material['motion-easing']).toBe('cubic-bezier(0.32, 0.72, 0, 1)');
  expect(palette).toContain('prefers-reduced-motion: reduce');
  expect(palette).toContain("[data-motion='reduced']");
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
  expect(geometry['traffic-light-clearance']).toBe('86px');
});
