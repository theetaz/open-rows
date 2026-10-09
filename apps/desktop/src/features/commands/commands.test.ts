// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  bindingsOf,
  commandForEvent,
  conflictOf,
  currentBindings,
  parseOverrides,
  rebind,
  resetBindings,
  shortcutText,
} from './bindings';
import { comboOf, displayCombo, formatCombo, parseCombo } from './keys';
import { COMMANDS } from './registry';
import { tabCommandOf } from './tabCommands';
import { menuStates } from './useMenuBridge';

const press = (
  key: string,
  code: string,
  modifiers: Partial<KeyboardEvent> = {},
): Parameters<typeof comboOf>[0] & { defaultPrevented: boolean } => ({
  key,
  code,
  metaKey: true,
  shiftKey: false,
  altKey: false,
  ctrlKey: false,
  defaultPrevented: false,
  ...modifiers,
});

it('parses, formats and displays shortcuts in macOS order', () => {
  const combo = parseCombo('Shift+Cmd+Enter');
  expect(combo).toEqual({
    code: 'Enter',
    meta: true,
    shift: true,
    alt: false,
    ctrl: false,
  });
  if (!combo) throw new Error('unparsed');
  expect(formatCombo(combo)).toBe('Shift+Cmd+Enter');
  expect(displayCombo(combo)).toBe('⇧⌘↵');
  expect(displayCombo(parseCombo('Ctrl+Alt+Cmd+T') ?? combo)).toBe('⌃⌥⌘T');
  expect(parseCombo('Cmd+Comma')?.code).toBe('Comma');
  for (const bad of ['T', 'Shift+T', 'Cmd+Nope', 'Hyper+Cmd+T', ''])
    expect(parseCombo(bad)).toBeNull();
  expect(parseCombo('F5')?.code).toBe('F5');
});

it('matches typed characters, and physical keys under Option', () => {
  expect(comboOf(press('t', 'KeyK'))?.code).toBe('KeyT');
  expect(comboOf(press('ˆ', 'KeyI', { altKey: true }))?.code).toBe('KeyI');
  expect(comboOf(press('Enter', ''))?.code).toBe('Enter');
  expect(comboOf(press('Meta', 'MetaLeft'))).toBeNull();
  expect(commandForEvent(press('t', 'KeyT'))).toBe('new-tab');
  expect(commandForEvent(press('Enter', 'Enter', { shiftKey: true }))).toBe(
    'run-all',
  );
});

it('rebinds, unbinds and resets shortcuts, keeping only valid overrides', () => {
  const runAll = parseCombo('Cmd+R');
  if (!runAll) throw new Error('unparsed');
  rebind('run-all', runAll);
  expect(currentBindings()['run-all']).toEqual(runAll);
  expect(commandForEvent(press('r', 'KeyR'))).toBe('refresh');
  rebind('refresh', null);
  expect(shortcutText(currentBindings(), 'refresh')).toBe('—');
  expect(shortcutText(currentBindings(), 'search')).toBe('⌘F in sidebar');
  resetBindings();
  expect(currentBindings()['run-all']).toEqual(parseCombo('Shift+Cmd+Enter'));
  expect(
    parseOverrides({ save: 'Cmd+J', nope: 'Cmd+K', open: 3, 'save-as': null }),
  ).toEqual({ save: 'Cmd+J', 'save-as': null });
});

it('detects conflicts with commands and reserved system shortcuts', () => {
  const bindings = bindingsOf({});
  const t = parseCombo('Cmd+T');
  const quit = parseCombo('Cmd+Q');
  const free = parseCombo('Cmd+J');
  if (!t || !quit || !free) throw new Error('unparsed');
  expect(conflictOf(bindings, 'save', t)).toEqual({
    kind: 'command',
    owner: 'New query tab',
    id: 'new-tab',
  });
  expect(conflictOf(bindings, 'new-tab', t)).toBeNull();
  expect(conflictOf(bindings, 'save', quit)).toMatchObject({
    kind: 'reserved',
    owner: 'Quit',
  });
  expect(conflictOf(bindings, 'save', free)).toBeNull();
  rebind('new-tab', null, undefined);
  rebind('save', t, 'new-tab');
  expect(currentBindings()['new-tab']).toBeNull();
  expect(commandForEvent(press('t', 'KeyT'))).toBe('save');
});

it('maps only unhandled tab shortcuts to tab commands', () => {
  expect(tabCommandOf(press('i', 'KeyI'))).toBe('beautify');
  expect(tabCommandOf(press('s', 'KeyS', { shiftKey: true }))).toBe('save-as');
  expect(tabCommandOf(press('t', 'KeyT'))).toBeUndefined();
  expect(
    tabCommandOf(press('Enter', 'Enter', { defaultPrevented: true })),
  ).toBeUndefined();
});

it('gives the menu bar every command with its accelerator and state', () => {
  const states = menuStates(bindingsOf({ cancel: null }), { save: false });
  expect(states).toHaveLength(COMMANDS.length);
  expect(states.find((item) => item.id === 'run-current')).toEqual({
    id: 'run-current',
    enabled: true,
    accelerator: 'Cmd+Enter',
  });
  expect(states.find((item) => item.id === 'save')?.enabled).toBe(false);
  expect(states.find((item) => item.id === 'cancel')?.accelerator).toBeNull();
});
