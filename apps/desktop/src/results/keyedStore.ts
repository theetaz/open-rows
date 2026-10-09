// SPDX-License-Identifier: Apache-2.0

/**
 * Device-local view state kept in web storage as one JSON object of entries
 * (grid layouts, table presentation). The least recently written entries
 * are dropped past `capacity`; a corrupt value resets every entry.
 */
type KeyedStore<T extends object> = {
  read: (key: string) => T | undefined;
  /** Store an entry; `undefined` forgets it. */
  write: (key: string, value: T | undefined) => void;
  subscribe: (listener: () => void) => () => void;
};

/** An entry's fields plus when it was last written. */
type Stored<T> = { value: T; used: number };

/** A keyed store under `storageKey`; `parse` validates one entry or drops it. */
export function keyedStore<T extends object>(
  storageKey: string,
  capacity: number,
  parse: (value: unknown) => T | null,
): KeyedStore<T> {
  const listeners = new Set<() => void>();
  let cache: {
    text: string | null;
    entries: Record<string, Stored<T>>;
  } | null = null;
  const all = (): Record<string, Stored<T>> => {
    const text = storedText(storageKey);
    if (cache?.text === text) return cache.entries;
    cache = { text, entries: parseAll(text, parse) };
    return cache.entries;
  };
  return {
    read: (key) => all()[key]?.value,
    write: (key, value) => {
      const entries = Object.fromEntries(
        Object.entries(all()).filter(([name]) => name !== key),
      );
      if (value !== undefined) entries[key] = { value, used: Date.now() };
      const kept = Object.fromEntries(
        Object.entries(entries)
          .sort(([, a], [, b]) => b.used - a.used)
          .slice(0, capacity),
      );
      const text = JSON.stringify(
        Object.fromEntries(
          Object.entries(kept).map(([name, entry]) => [
            name,
            { ...entry.value, used: entry.used },
          ]),
        ),
      );
      try {
        localStorage.setItem(storageKey, text);
        cache = null;
      } catch {
        cache = { text: storedText(storageKey), entries: kept };
      }
      listeners.forEach((listener) => {
        listener();
      });
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

function storedText(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function parseAll<T extends object>(
  text: string | null,
  parse: (value: unknown) => T | null,
): Record<string, Stored<T>> {
  const entries: Record<string, Stored<T>> = {};
  try {
    const parsed: unknown = text ? JSON.parse(text) : {};
    if (typeof parsed !== 'object' || parsed === null) return entries;
    for (const [key, raw] of Object.entries(parsed)) {
      const value = parse(raw);
      if (value === null) continue;
      const used = (raw as { used?: unknown }).used;
      entries[key] = { value, used: typeof used === 'number' ? used : 0 };
    }
  } catch {
    /* A corrupt store resets every entry to the defaults. */
  }
  return entries;
}
