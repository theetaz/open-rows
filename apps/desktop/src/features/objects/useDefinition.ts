// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';

/** A definition being read, read, or why it could not be. */
export type Loaded<T> =
  | { status: 'loading' }
  | { status: 'ready'; value: T }
  | { status: 'error'; message: string };

/**
 * The starting text of an editor for the object `key` names: `load()` for
 * an existing object, read once; a template right away for a new one.
 */
export function useDefinition<T>(
  key: string,
  load: { read: () => Promise<T> } | { value: T },
): Loaded<T> {
  const [state, setState] = useState<Loaded<T>>(
    'value' in load
      ? { status: 'ready', value: load.value }
      : { status: 'loading' },
  );
  const latest = useRef(load);
  useEffect(() => {
    latest.current = load;
  });
  useEffect(() => {
    const source = latest.current;
    if (!('read' in source)) return;
    let current = true;
    source.read().then(
      (value) => {
        if (current) setState({ status: 'ready', value });
      },
      (error: unknown) => {
        if (current)
          setState({
            status: 'error',
            message: error instanceof Error ? error.message : String(error),
          });
      },
    );
    return () => {
      current = false;
    };
  }, [key]);
  return state;
}
