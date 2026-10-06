// SPDX-License-Identifier: Apache-2.0
import '@testing-library/jest-dom/vitest';
import { useState } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { Button, IconButton } from './Button';
import { SegmentedControl } from './SegmentedControl';
import { Switch } from './Switch';
import { TextField } from './TextField';

const options = [
  { value: 'one', label: 'One' },
  { value: 'two', label: 'Two' },
  { value: 'three', label: 'Three' },
] as const;
function Selection(): React.JSX.Element {
  const [value, setValue] = useState('one');
  return (
    <SegmentedControl
      aria-label="Choice"
      options={options}
      value={value}
      onValueChange={setValue}
    />
  );
}
it('arrows/Home/End select, focus and maintain one tab stop', async () => {
  const user = userEvent.setup();
  render(<Selection />);
  await user.tab();
  const sequence = [
    ['{ArrowRight}', 'Two'],
    ['{ArrowDown}', 'Three'],
    ['{ArrowRight}', 'One'],
    ['{ArrowLeft}', 'Three'],
    ['{Home}', 'One'],
    ['{End}', 'Three'],
    ['{ArrowUp}', 'Two'],
  ];
  for (const [key, label] of sequence) {
    await user.keyboard(key ?? '');
    const selected = screen.getByRole('radio', { name: label ?? '' });
    expect(selected).toHaveFocus();
    expect(selected).toHaveAttribute('aria-checked', 'true');
    expect(
      screen.getAllByRole('radio').filter((item) => item.tabIndex === 0),
    ).toEqual([selected]);
  }
});
it('skips disabled segments, supports clicks and disables the complete group', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  const { rerender } = render(
    <SegmentedControl
      aria-label="Choice"
      options={[
        options[0],
        { value: 'two', label: 'Two', disabled: true },
        options[2],
      ]}
      value="one"
      onValueChange={change}
    />,
  );
  await user.tab();
  await user.keyboard('{ArrowRight}');
  expect(change).toHaveBeenLastCalledWith('three');
  await user.click(screen.getByRole('radio', { name: 'One' }));
  expect(change).toHaveBeenLastCalledWith('one');
  rerender(
    <SegmentedControl
      aria-label="Choice"
      options={options}
      value="one"
      onValueChange={change}
      disabled
    />,
  );
  expect(
    screen.getAllByRole('radio').every((item) => item.tabIndex === -1),
  ).toBe(true);
});
function Toggle(): React.JSX.Element {
  const [checked, setChecked] = useState(false);
  return (
    <Switch
      aria-label="Safe mode"
      checked={checked}
      onCheckedChange={setChecked}
    />
  );
}
it('switch toggles with Space, Enter and pointer activation', async () => {
  const user = userEvent.setup();
  render(<Toggle />);
  await user.tab();
  const toggle = screen.getByRole('switch', { name: 'Safe mode' });
  await user.keyboard(' ');
  expect(toggle).toHaveAttribute('aria-checked', 'true');
  await user.keyboard('{Enter}');
  expect(toggle).toHaveAttribute('aria-checked', 'false');
  await user.click(toggle);
  expect(toggle).toHaveAttribute('aria-checked', 'true');
});
it('labels fields, links errors and preserves additional descriptions', () => {
  render(
    <>
      <span id="hint">Database URL</span>
      <TextField
        label="Connection"
        error="Invalid URL"
        aria-describedby="hint"
      />
      <TextField aria-label="Search" />
    </>,
  );
  expect(screen.getByLabelText('Connection')).toHaveAttribute(
    'aria-invalid',
    'true',
  );
  expect(screen.getByLabelText('Connection')).toHaveAccessibleDescription(
    'Database URL Invalid URL',
  );
  expect(screen.getByRole('textbox', { name: 'Search' })).toBeInTheDocument();
});
it('disabled actions do not activate', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(
    <>
      <Button onClick={change} disabled>
        Run
      </Button>
      <IconButton aria-label="Refresh" onClick={change} disabled />
      <Switch
        aria-label="Safe mode"
        checked={false}
        onCheckedChange={change}
        disabled
      />
    </>,
  );
  for (const control of [
    ...screen.getAllByRole('button'),
    screen.getByRole('switch'),
  ])
    await user.click(control);
  expect(change).not.toHaveBeenCalled();
});
it('requires IconButton and TextField names at compile time', () => {
  // @ts-expect-error An icon-only button cannot omit its accessible name.
  const unnamedIcon = <IconButton />;
  // @ts-expect-error A field needs either a visible label or aria-label.
  const unnamedField = <TextField />;
  expect(unnamedIcon).toBeDefined();
  expect(unnamedField).toBeDefined();
});
