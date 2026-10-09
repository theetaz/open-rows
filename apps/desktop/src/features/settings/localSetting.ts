// SPDX-License-Identifier: Apache-2.0
import { useSyncExternalStore } from 'react';

/** Called after any setting changed, to rewrite `settings.json`. */
let changed: (() => void) | null = null;

/** Register the one listener that persists settings to the file. */
export function onSettingsChanged(listener: (() => void) | null): void {
  changed = listener;
}

/** Report a changed setting (stores in other packages call this too). */
export function settingsChanged(): void {
  changed?.();
}

/** A setting kept in web storage under `key`, shared live by every reader. */
type LocalSetting<T> = {
  get: () => T;
  set: (value: T) => void;
  use: () => T;
};

/**
 * A validated setting stored as JSON in web storage (the synchronous cache
 * of `settings.json`); without storage it lasts for this session.
 */
export function localSetting<T>(
  key: string,
  parse: (stored: unknown) => T,
): LocalSetting<T> {
  const listeners = new Set<() => void>();
  let cached: { text: string | null; value: T } | null = null;
  let unstored: { value: T } | null = null;
  const get = (): T => {
    if (unstored) return unstored.value;
    let text: string | null = null;
    try {
      text = localStorage.getItem(key);
    } catch {
      text = null;
    }
    if (cached?.text !== text) cached = { text, value: parse(decode(text)) };
    return cached.value;
  };
  const set = (value: T): void => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
      unstored = null;
    } catch {
      unstored = { value };
    }
    listeners.forEach((listener) => {
      listener();
    });
    settingsChanged();
  };
  const subscribe = (listener: () => void): (() => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };
  return { get, set, use: () => useSyncExternalStore(subscribe, get) };
}

function decode(text: string | null): unknown {
  try {
    return text ? (JSON.parse(text) as unknown) : null;
  } catch {
    return null;
  }
}
