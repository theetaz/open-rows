// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { Tabs } from './Tabs';

const tabs = [
  { id: 'a', label: 'orders •', name: 'orders' },
  { id: 'b', label: 'query' },
];

function Renamable(props: {
  onRename: (id: string, title: string | null) => void;
  onKeep: (id: string) => void;
}): React.JSX.Element {
  const [renaming, setRenaming] = useState<string | null>(null);
  return (
    <Tabs
      tabs={tabs}
      active="a"
      onSelect={vi.fn()}
      onClose={vi.fn()}
      onAdd={vi.fn()}
      onKeep={props.onKeep}
      renaming={renaming}
      onRenameStart={setRenaming}
      onRename={(id, title) => {
        setRenaming(null);
        props.onRename(id, title);
      }}
    />
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

it('renames a tab in place on double-click: Enter keeps, Escape cancels', async () => {
  const rename = vi.fn();
  const keep = vi.fn();
  const user = userEvent.setup();
  render(<Renamable onRename={rename} onKeep={keep} />);
  await user.dblClick(screen.getByRole('tab', { name: /orders/ }));
  expect(keep).toHaveBeenCalledWith('a');
  const field = screen.getByRole('textbox', { name: 'Tab name' });
  expect(field).toHaveFocus();
  expect(field).toHaveValue('orders');
  await user.clear(field);
  await user.type(field, '  Monthly orders {Enter}');
  expect(rename).toHaveBeenLastCalledWith('a', 'Monthly orders');
  expect(screen.queryByRole('textbox', { name: 'Tab name' })).toBeNull();
  await user.dblClick(screen.getByRole('tab', { name: 'query' }));
  await user.type(
    screen.getByRole('textbox', { name: 'Tab name' }),
    'x{Escape}',
  );
  expect(rename).toHaveBeenLastCalledWith('b', null);
  expect(rename).toHaveBeenCalledTimes(2);
});

it('shows the overflow button only while the tabs do not fit', async () => {
  const overflow = vi.fn();
  const props = {
    tabs,
    active: 'a',
    onSelect: vi.fn(),
    onClose: vi.fn(),
    onAdd: vi.fn(),
    onOverflow: overflow,
  };
  const view = render(<Tabs {...props} />);
  expect(screen.queryByRole('button', { name: 'All tabs' })).toBeNull();
  vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockReturnValue(900);
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(300);
  view.rerender(
    <Tabs {...props} tabs={[...tabs, { id: 'c', label: 'more' }]} />,
  );
  await userEvent
    .setup()
    .click(screen.getByRole('button', { name: 'All tabs' }));
  expect(overflow).toHaveBeenCalledOnce();
});
