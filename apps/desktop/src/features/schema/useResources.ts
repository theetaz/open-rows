// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';

/** One cached catalog read; `refreshing` while a background re-read runs. */
export type Resource<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'ready'; data: T; refreshing: boolean }
  | { status: 'error'; message: string };

/** A read from the schema cache: `stale` data is shown, then read again. */
export type CachedRead<T> = { data: T; stale: boolean };

/** Reads one resource; `fresh` bypasses the schema cache. */
export type Fetch<T> = (fresh: boolean) => Promise<CachedRead<T>>;

const idle: Resource<never> = { status: 'idle' };

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

type States = Readonly<Record<string, Resource<unknown> | undefined>>;

export type Resources = {
  get: <T>(key: string) => Resource<T>;
  load: <T>(key: string, fetch: Fetch<T>) => void;
  /** Forget every read under `prefix`; it is read again when next shown. */
  invalidate: (prefix: string) => void;
  /** Read every loaded resource under `prefix` again, keeping its data. */
  revalidate: (prefix: string) => void;
};

/**
 * Keyed reads, each fetched at most once until invalidated or revalidated.
 * Stale replies are shown at once and re-read fresh in the background;
 * results of reads superseded by a newer one are discarded.
 */
class ResourceTable {
  private readonly states = new Map<string, Resource<unknown>>();
  private readonly fetchers = new Map<string, Fetch<unknown>>();
  private readonly tokens = new Map<string, number>();
  private counter = 0;

  constructor(private readonly changed: (states: States) => void) {}

  load(key: string, fetch: Fetch<unknown>): void {
    if (this.tokens.has(key)) return;
    this.fetchers.set(key, fetch);
    this.run(key, false);
  }

  invalidate(prefix: string): void {
    for (const key of this.keys(prefix)) this.forget(key);
    this.publish();
  }

  revalidate(prefix: string): void {
    for (const key of this.keys(prefix)) {
      if (this.states.get(key)?.status === 'error') this.forget(key);
      else this.run(key, true);
    }
    this.publish();
  }

  private keys(prefix: string): string[] {
    return [...this.tokens.keys()].filter((key) => key.startsWith(prefix));
  }

  private forget(key: string): void {
    this.tokens.delete(key);
    this.states.delete(key);
    this.fetchers.delete(key);
  }

  private run(key: string, fresh: boolean): void {
    const fetch = this.fetchers.get(key);
    if (!fetch) return;
    const token = ++this.counter;
    this.tokens.set(key, token);
    const current = this.states.get(key);
    this.set(
      key,
      current?.status === 'ready'
        ? { ...current, refreshing: true }
        : { status: 'loading' },
    );
    const live = (): boolean => this.tokens.get(key) === token;
    fetch(fresh).then(
      ({ data, stale }) => {
        if (!live()) return;
        const again = stale && !fresh;
        this.set(key, { status: 'ready', data, refreshing: again });
        if (again) this.run(key, true);
      },
      (error: unknown) => {
        if (!live()) return;
        const kept = this.states.get(key);
        this.set(
          key,
          kept?.status === 'ready'
            ? { ...kept, refreshing: false }
            : { status: 'error', message: message(error) },
        );
      },
    );
  }

  private set(key: string, resource: Resource<unknown>): void {
    this.states.set(key, resource);
    this.publish();
  }

  private publish(): void {
    this.changed(Object.fromEntries(this.states));
  }
}

/** Keyed async catalog reads with stale-while-refresh semantics. */
export function useResources(): Resources {
  const [states, setStates] = useState<States>({});
  const [table] = useState(() => new ResourceTable(setStates));
  const get = useCallback(
    <T>(key: string): Resource<T> =>
      (states[key] as Resource<T> | undefined) ?? idle,
    [states],
  );
  const load = useCallback(
    <T>(key: string, fetch: Fetch<T>): void => {
      table.load(key, fetch);
    },
    [table],
  );
  const invalidate = useCallback(
    (prefix: string): void => {
      table.invalidate(prefix);
    },
    [table],
  );
  const revalidate = useCallback(
    (prefix: string): void => {
      table.revalidate(prefix);
    },
    [table],
  );
  return { get, load, invalidate, revalidate };
}

/** A ready resource re-reading in the background. */
export function isRefreshing(resource: Resource<unknown>): boolean {
  return resource.status === 'ready' && resource.refreshing;
}
