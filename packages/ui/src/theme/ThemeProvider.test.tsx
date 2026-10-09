// SPDX-License-Identifier: Apache-2.0
import '@testing-library/jest-dom/vitest';
import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ThemeProvider, useTheme } from './ThemeProvider';

function Consumer(): React.JSX.Element {
  const { mode, resolved, setMode } = useTheme();
  return (
    <>
      <output aria-label="Theme">
        {mode}/{resolved}
      </output>
      {(['system', 'light', 'dark'] as const).map((next) => (
        <button
          key={next}
          onClick={() => {
            setMode(next);
          }}
        >
          {next}
        </button>
      ))}
    </>
  );
}
it('applies light/dark/system and persists through provider remounts', async () => {
  const user = userEvent.setup();
  const first = render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  expect(document.documentElement).toHaveAttribute('data-theme', 'system');
  await user.click(screen.getByText('dark'));
  expect(localStorage.getItem('openrows.theme')).toBe('dark');
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  first.unmount();
  render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  expect(screen.getByLabelText('Theme')).toHaveTextContent('dark/dark');
  await user.click(screen.getByText('light'));
  expect(document.documentElement).toHaveAttribute('data-theme', 'light');
  await user.click(screen.getByText('system'));
  expect(localStorage.getItem('openrows.theme')).toBe('system');
});
it('follows OS changes in system mode, retains explicit appearance, removes listener', async () => {
  const user = userEvent.setup();
  const media = {
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => media),
  );
  const view = render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  const listener = media.addEventListener.mock.calls[0]?.[1] as (event: {
    matches: boolean;
  }) => void;
  act(() => {
    listener({ matches: true });
  });
  expect(screen.getByLabelText('Theme')).toHaveTextContent('system/dark');
  await user.click(screen.getByText('light'));
  act(() => {
    listener({ matches: false });
  });
  expect(screen.getByLabelText('Theme')).toHaveTextContent('light/light');
  view.unmount();
  expect(media.removeEventListener).toHaveBeenCalledWith('change', listener);
});
it('tolerates unavailable storage and rejects invalid stored values', async () => {
  localStorage.setItem('openrows.theme', 'invalid');
  const get = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
    throw new DOMException('Denied');
  });
  const set = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new DOMException('Denied');
  });
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  expect(screen.getByLabelText('Theme')).toHaveTextContent('system/light');
  await user.click(screen.getByText('dark'));
  expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
  get.mockRestore();
  set.mockRestore();
});
it('writes only one root attribute and never reads layout when switching theme', async () => {
  const user = userEvent.setup();
  render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  const records: MutationRecord[] = [];
  const observer = new MutationObserver((changes) => {
    records.push(...changes);
  });
  observer.observe(document.documentElement, {
    attributes: true,
    subtree: true,
  });
  const measure = vi.spyOn(Element.prototype, 'getBoundingClientRect');
  await user.click(screen.getByText('dark'));
  observer.disconnect();
  expect(records.filter((record) => record.type === 'attributes')).toHaveLength(
    1,
  );
  expect(records[0]?.attributeName).toBe('data-theme');
  expect(measure).not.toHaveBeenCalled();
  measure.mockRestore();
});

it('falls back from an invalid preference and resolves an initially dark OS', () => {
  localStorage.setItem('openrows.theme', 'unknown');
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({
      matches: true,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
  render(
    <ThemeProvider>
      <Consumer />
    </ThemeProvider>,
  );
  expect(screen.getByLabelText('Theme')).toHaveTextContent('system/dark');
  expect(document.documentElement).toHaveAttribute('data-theme', 'system');
});
it('re-reads stored preferences when the revision changes, without onChange', () => {
  const onChange = vi.fn();
  const view = render(
    <ThemeProvider onChange={onChange} revision={0}>
      <Consumer />
    </ThemeProvider>,
  );
  expect(screen.getByLabelText('Theme')).toHaveTextContent('system/');
  localStorage.setItem('openrows.theme', 'dark');
  view.rerender(
    <ThemeProvider onChange={onChange} revision={1}>
      <Consumer />
    </ThemeProvider>,
  );
  expect(screen.getByLabelText('Theme')).toHaveTextContent('dark/dark');
  expect(onChange).not.toHaveBeenCalled();
});
