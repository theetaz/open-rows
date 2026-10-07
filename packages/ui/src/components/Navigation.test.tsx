// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { Tabs } from './Tabs';
import { DisclosureSection } from './DisclosureSection';
import { TreeItem } from './TreeItem';
import { Sheet } from './Sheet';

it('document tabs wrap keyboard focus and support Home, End, close and add', async () => {
  const select = vi.fn();
  const close = vi.fn();
  const add = vi.fn();
  const user = userEvent.setup();
  render(
    <Tabs
      tabs={[
        { id: 'a', label: 'orders' },
        { id: 'b', label: 'query' },
      ]}
      active="a"
      onSelect={select}
      onClose={close}
      onAdd={add}
    />,
  );
  const orders = screen.getByRole('tab', { name: /orders/ });
  orders.focus();
  await user.keyboard('{ArrowRight}');
  expect(select).toHaveBeenLastCalledWith('b');
  expect(screen.getByRole('tab', { name: /query/ })).toHaveFocus();
  await user.keyboard('{ArrowRight}');
  expect(select).toHaveBeenLastCalledWith('a');
  await user.keyboard('{End}');
  expect(select).toHaveBeenLastCalledWith('b');
  await user.keyboard('{Home}');
  expect(select).toHaveBeenLastCalledWith('a');
  fireEvent(orders, new MouseEvent('auxclick', { button: 1, bubbles: true }));
  expect(close).toHaveBeenCalledWith('a');
  await user.click(screen.getByRole('button', { name: 'Close query' }));
  expect(close).toHaveBeenCalledWith('b');
  await user.click(screen.getByRole('button', { name: 'New tab' }));
  expect(add).toHaveBeenCalledOnce();
});
it('disclosures make collapsed children inert and tree rows activate by keyboard', async () => {
  const select = vi.fn();
  const user = userEvent.setup();
  render(
    <DisclosureSection label="Tables" count={1}>
      <TreeItem label="orders" count="2.4M" selected onSelect={select} />
    </DisclosureSection>,
  );
  const section = screen.getByRole('button', { name: /Tables/ });
  const children = document.getElementById(
    section.getAttribute('aria-controls') ?? '',
  );
  expect(children).toHaveAttribute('inert');
  section.focus();
  await user.keyboard('{Enter}');
  expect(children).not.toHaveAttribute('inert');
  const item = screen.getByRole('button', { name: /orders/ });
  item.focus();
  await user.keyboard(' ');
  expect(select).toHaveBeenCalledOnce();
  expect(item).toHaveAttribute('aria-pressed', 'true');
});
it('lazy disclosures report toggles and openable rows open on double-click or Enter', async () => {
  const toggle = vi.fn();
  const select = vi.fn();
  const open = vi.fn();
  const user = userEvent.setup();
  render(
    <DisclosureSection label="Views" onOpenChange={toggle}>
      <TreeItem
        label="summary"
        glyph="▤"
        selected={false}
        onSelect={select}
        onOpen={open}
      />
    </DisclosureSection>,
  );
  const section = screen.getByRole('button', { name: 'Views' });
  await user.click(section);
  expect(toggle).toHaveBeenLastCalledWith(true);
  const item = screen.getByRole('button', { name: 'summary' });
  await user.click(item);
  expect(select).toHaveBeenCalledOnce();
  expect(open).not.toHaveBeenCalled();
  await user.dblClick(item);
  expect(open).toHaveBeenCalledOnce();
  item.focus();
  await user.keyboard('{Enter}');
  expect(open).toHaveBeenCalledTimes(2);
  await user.click(section);
  expect(toggle).toHaveBeenLastCalledWith(false);
});
it('sheet contains focus, closes on Escape and restores background inert state', async () => {
  const close = vi.fn();
  const user = userEvent.setup();
  const { unmount } = render(
    <div>
      <button type="button">Background</button>
      <Sheet title="Review" onClose={close}>
        <button type="button">Last</button>
      </Sheet>
    </div>,
  );
  const first = screen.getByRole('button', { name: 'Close sheet' });
  const last = screen.getByRole('button', { name: 'Last' });
  expect(first).toHaveFocus();
  await user.keyboard('{Shift>}{Tab}{/Shift}');
  expect(last).toHaveFocus();
  await user.keyboard('{Tab}');
  expect(first).toHaveFocus();
  const background = screen.getByRole('button', { name: 'Background' });
  expect(background).toHaveAttribute('inert');
  await user.keyboard('{Escape}');
  expect(close).toHaveBeenCalledOnce();
  unmount();
  expect(background).not.toHaveAttribute('inert');
});
it('sheet uses native showModal/close when available and handles cancel', () => {
  const show = vi.fn(function (this: HTMLDialogElement) {
    this.setAttribute('open', '');
  });
  const hide = vi.fn();
  const close = vi.fn();
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', {
    configurable: true,
    value: show,
  });
  Object.defineProperty(HTMLDialogElement.prototype, 'close', {
    configurable: true,
    value: hide,
  });
  const { unmount } = render(
    <Sheet title="Native" onClose={close}>
      Changes
    </Sheet>,
  );
  expect(show).toHaveBeenCalledOnce();
  fireEvent(
    screen.getByRole('dialog'),
    new Event('cancel', { cancelable: true }),
  );
  expect(close).toHaveBeenCalledOnce();
  unmount();
  expect(hide).toHaveBeenCalledOnce();
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
  Reflect.deleteProperty(HTMLDialogElement.prototype, 'close');
});
