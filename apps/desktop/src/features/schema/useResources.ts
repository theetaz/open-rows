// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';

/** One cached catalog read. */
export type Resource<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ready'; data: T }
  | { status: 'error'; message: string };

const idle: Resource<never> = { status: 'idle' };

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

export type Resources = {
  get: <T>(key: string) => Resource<T>;
  load: <T>(key: string, fetch: () => Promise<T>) => void;
  invalidate: (prefix: string) => void;
};

/**
 * Keyed async reads, each fetched at most once until invalidated. Results of
 * reads started before an invalidation are discarded.
 */
export function useResources(): Resources {
  const [entries, setEntries] = useState<
    Readonly<Record<string, Resource<unknown> | undefined>>
  >({});
  const tokens = useRef(new Map<string, number>());
  const counter = useRef(0);
  const load = useCallback(<T>(key: string, fetch: () => Promise<T>): void => {
    if (tokens.current.has(key)) return;
    const token = ++counter.current;
    tokens.current.set(key, token);
    const settle = (resource: Resource<T>): void => {
      if (tokens.current.get(key) !== token) return;
      setEntries((current) => ({ ...current, [key]: resource }));
    };
    settle({ status: 'loading' });
    fetch().then(
      (data) => {
        settle({ status: 'ready', data });
      },
      (error: unknown) => {
        settle({ status: 'error', message: message(error) });
      },
    );
  }, []);
  const invalidate = useCallback((prefix: string): void => {
    for (const key of [...tokens.current.keys()])
      if (key.startsWith(prefix)) tokens.current.delete(key);
    setEntries((current) =>
      Object.fromEntries(
        Object.entries(current).filter(([key]) => !key.startsWith(prefix)),
      ),
    );
  }, []);
  const get = useCallback(
    <T>(key: string): Resource<T> =>
      (entries[key] as Resource<T> | undefined) ?? idle,
    [entries],
  );
  return { get, load, invalidate };
}
