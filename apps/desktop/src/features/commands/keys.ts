// SPDX-License-Identifier: Apache-2.0
/**
 * A key combination: a physical key (`KeyboardEvent.code`, so Option's
 * altered characters still match) and exact modifiers.
 */
export type KeyCombo = {
  code: string;
  meta: boolean;
  shift: boolean;
  alt: boolean;
  ctrl: boolean;
};

type KeyInput = Pick<
  KeyboardEvent,
  'key' | 'code' | 'metaKey' | 'shiftKey' | 'altKey' | 'ctrlKey'
>;

/** Non-letter, non-digit keys a shortcut may use, with their menu glyphs. */
const NAMED: Readonly<Record<string, string>> = {
  Enter: '↵',
  Comma: ',',
  Period: '.',
  Slash: '/',
  Backslash: '\\',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Quote: "'",
  Minus: '-',
  Equal: '=',
  Backquote: '`',
  Backspace: '⌫',
  Delete: '⌦',
  Escape: '⎋',
  Tab: '⇥',
  Space: 'Space',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  ...Object.fromEntries(
    Array.from({ length: 12 }, (_, index) => [
      `F${String(index + 1)}`,
      `F${String(index + 1)}`,
    ]),
  ),
};

/** `KeyT` → `T`, `Digit1` → `1`, named keys as they are. */
function keyName(code: string): string {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return code;
}

/** `T` → `KeyT`, `1` → `Digit1`; `null` for keys shortcuts cannot use. */
function codeOf(name: string): string | null {
  if (/^[A-Z]$/i.test(name)) return `Key${name.toUpperCase()}`;
  if (/^\d$/.test(name)) return `Digit${name}`;
  return Object.hasOwn(NAMED, name) ? name : null;
}

const MODIFIERS = {
  ctrl: 'Ctrl',
  alt: 'Alt',
  shift: 'Shift',
  meta: 'Cmd',
} as const;

/** The stored and accelerator form, e.g. `Shift+Cmd+Enter` (Tauri syntax). */
export function formatCombo(combo: KeyCombo): string {
  const parts = Object.entries(MODIFIERS)
    .filter(([flag]) => combo[flag as keyof typeof MODIFIERS])
    .map(([, name]) => name);
  return [...parts, keyName(combo.code)].join('+');
}

/** Parse `Shift+Cmd+Enter`; `null` when malformed or without ⌘/⌃. */
export function parseCombo(text: string): KeyCombo | null {
  const parts = text.split('+').map((part) => part.trim());
  const key = parts.pop();
  const code = key ? codeOf(key) : null;
  if (!code) return null;
  const known = new Set<string>(Object.values(MODIFIERS));
  if (!parts.every((part) => known.has(part))) return null;
  const has = (name: string): boolean => parts.includes(name);
  const combo = {
    code,
    meta: has('Cmd'),
    shift: has('Shift'),
    alt: has('Alt'),
    ctrl: has('Ctrl'),
  };
  return combo.meta || combo.ctrl || /^F\d+$/.test(code) ? combo : null;
}

/** macOS menu notation in the standard order, e.g. `⌃⌥⇧⌘↵`. */
export function displayCombo(combo: KeyCombo): string {
  const key = NAMED[combo.code] ?? keyName(combo.code);
  return `${combo.ctrl ? '⌃' : ''}${combo.alt ? '⌥' : ''}${combo.shift ? '⇧' : ''}${combo.meta ? '⌘' : ''}${key}`;
}

/**
 * The key of a press. Like macOS key equivalents, the typed character wins
 * (so ⌘T follows the keyboard layout); Option alters characters, so with ⌥
 * (or for characters Shift changed) the physical key is used.
 */
function physical(event: KeyInput): string | null {
  const glyph = Object.entries(NAMED).find(([, value]) => value === event.key);
  const typed = event.altKey ? null : (codeOf(event.key) ?? glyph?.[0]);
  return typed ?? (event.code ? codeOf(keyName(event.code)) : null);
}

/** The combination a key press makes; `null` for bare modifiers or unknown keys. */
export function comboOf(event: KeyInput): KeyCombo | null {
  const code = physical(event);
  if (!code) return null;
  return {
    code,
    meta: event.metaKey,
    shift: event.shiftKey,
    alt: event.altKey,
    ctrl: event.ctrlKey,
  };
}
