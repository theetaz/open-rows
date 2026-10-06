// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { beforeEach, expect, it, vi } from 'vitest';
import { SplitPanel } from './SplitPanel';
import { boundedWidth, readWidth, writeWidth } from './width';

const range = { min: 200, max: 360, initial: 228 };
beforeEach(() => {
  vi.stubGlobal('PointerEvent', MouseEvent);
  HTMLElement.prototype.setPointerCapture = vi.fn();
});
function panel(edge: 'left' | 'right' = 'right'): React.JSX.Element {
  return (
    <SplitPanel
      label="Sidebar"
      storageKey="test.sidebar"
      range={range}
      open
      edge={edge}
    >
      Items
    </SplitPanel>
  );
}
it('clamps corrupted persisted values and tolerates unavailable storage', () => {
  expect(readWidth('none', range)).toBe(228);
  for (const [value, expected] of [
    ['NaN', 228],
    ['9999', 360],
    ['-5', 200],
  ] as const) {
    localStorage.setItem('width', value);
    expect(readWidth('width', range)).toBe(expected);
  }
  expect(boundedWidth(Infinity, range)).toBe(228);
  const get = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
    throw new Error('Blocked');
  });
  expect(readWidth('width', range)).toBe(228);
  get.mockRestore();
  const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new Error('Blocked');
  });
  expect(() => {
    writeWidth('width', 240);
  }).not.toThrow();
  set.mockRestore();
});
it('previews directly, clamps both bounds, saves only on release and restores on remount', () => {
  const { unmount } = render(panel());
  const handle = screen.getByRole('separator');
  const region = screen.getByRole('complementary');
  fireEvent.pointerDown(handle, { clientX: 228, button: 0 });
  fireEvent.pointerMove(handle, { clientX: 999 });
  expect(region.style.getPropertyValue('--panel-width')).toBe('360px');
  expect(localStorage.getItem('test.sidebar')).toBeNull();
  fireEvent.pointerUp(handle);
  expect(localStorage.getItem('test.sidebar')).toBe('360');
  fireEvent.pointerDown(handle, { clientX: 360, button: 0 });
  fireEvent.pointerMove(handle, { clientX: 0 });
  fireEvent.pointerUp(handle);
  expect(handle).toHaveAttribute('aria-valuenow', '200');
  unmount();
  render(panel());
  expect(screen.getByRole('separator')).toHaveAttribute('aria-valuenow', '200');
  fireEvent.doubleClick(screen.getByRole('separator'));
  expect(localStorage.getItem('test.sidebar')).toBe('228');
});
it('inspector drag direction, keyboard bounds and cancellation share the same logic', () => {
  render(
    <SplitPanel
      label="Inspector"
      storageKey="test.inspector"
      range={{ min: 280, max: 440, initial: 316 }}
      open
      edge="left"
    >
      Details
    </SplitPanel>,
  );
  const handle = screen.getByRole('separator');
  fireEvent.pointerDown(handle, { clientX: 800, button: 0 });
  fireEvent.pointerMove(handle, { clientX: 0 });
  fireEvent.pointerUp(handle);
  expect(handle).toHaveAttribute('aria-valuenow', '440');
  fireEvent.pointerDown(handle, { clientX: 800, button: 0 });
  fireEvent.pointerMove(handle, { clientX: 9999 });
  fireEvent.pointerUp(handle);
  expect(handle).toHaveAttribute('aria-valuenow', '280');
  fireEvent.keyDown(handle, { key: 'Enter' });
  expect(handle).toHaveAttribute('aria-valuenow', '316');
  fireEvent.keyDown(handle, { key: 'ArrowLeft' });
  expect(handle).toHaveAttribute('aria-valuenow', '326');
  fireEvent.keyDown(handle, { key: 'Home' });
  expect(handle).toHaveAttribute('aria-valuenow', '280');
  fireEvent.keyDown(handle, { key: 'End' });
  expect(handle).toHaveAttribute('aria-valuenow', '440');
  fireEvent.pointerDown(handle, { clientX: 800, button: 0 });
  fireEvent.pointerMove(handle, { clientX: 900 });
  fireEvent.pointerCancel(handle);
  expect(handle).toHaveAttribute('aria-valuenow', '440');
  expect(screen.getByRole('complementary')).not.toHaveAttribute(
    'data-resizing',
  );
});
it('collapses to an inert region and uses the shared reduced-motion timing tokens', () => {
  const { rerender } = render(panel());
  rerender(
    <SplitPanel
      label="Sidebar"
      storageKey="test.sidebar"
      range={range}
      open={false}
      edge="right"
    >
      Items
    </SplitPanel>,
  );
  expect(screen.getByLabelText('Sidebar')).toHaveAttribute('inert');
  expect(screen.getByLabelText('Sidebar')).toHaveAttribute(
    'data-open',
    'false',
  );
  const css = readFileSync(
    'packages/ui/src/components/split/SplitPanel.module.css',
    'utf8',
  );
  expect(css).toContain('var(--or-motion-panel) var(--or-motion-easing)');
  const tokens = readFileSync('packages/ui/src/tokens/tokens.css', 'utf8');
  expect(tokens).toContain('--or-motion-panel: 360ms');
  expect(tokens).toContain('cubic-bezier(0.32, 0.72, 0, 1)');
  expect(tokens).toContain('@media (prefers-reduced-motion: reduce)');
  expect(tokens).toContain('--or-motion-panel: 0ms');
});

it('unmounts closed content at 360 ms and cancels exit when reopened', () => {
  vi.useFakeTimers();
  try {
    const renderPanel = (open: boolean): React.JSX.Element => (
      <SplitPanel
        label="Inspector"
        storageKey="test.presence"
        range={range}
        open={open}
        edge="left"
        overlay
      >
        <p>Panel content</p>
      </SplitPanel>
    );
    const { rerender } = render(renderPanel(false));
    expect(screen.queryByText('Panel content')).toBeNull();
    rerender(renderPanel(true));
    rerender(renderPanel(false));
    act(() => {
      vi.advanceTimersByTime(359);
    });
    expect(screen.getByText('Panel content')).toBeInTheDocument();
    rerender(renderPanel(true));
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.getByText('Panel content')).toBeVisible();
    rerender(renderPanel(false));
    act(() => {
      vi.advanceTimersByTime(360);
    });
    expect(screen.queryByText('Panel content')).toBeNull();
    expect(document.querySelector('[aria-label="Inspector"]')).toBeNull();
  } finally {
    vi.useRealTimers();
  }
});
