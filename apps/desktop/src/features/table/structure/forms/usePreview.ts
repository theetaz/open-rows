// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { tableAlterPreview } from '@openrows/ipc';
import type { SchemaChangeDto } from '@openrows/ipc';
import type { TableTargetRef } from '../../model';

/** Generated statements and their warnings. */
export type Generated = {
  statements: readonly string[];
  warnings: readonly string[];
};

/** A generated preview (with whatever else came with it), while loading or failed. */
export type PreviewState<T extends Generated = Generated> =
  | { status: 'idle' }
  | { status: 'loading' }
  | ({ status: 'ready' } & T)
  | { status: 'error'; message: string };

/** Wait this long after the last edit before generating again. */
const SETTLE_MS = 200;

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Generate a preview for the input identified by `key` (`null`: nothing
 * to preview) once edits settle; a newer input supersedes an older one.
 */
export function usePreview<T extends Generated>(
  key: string | null,
  generate: () => Promise<T>,
): PreviewState<T> {
  const [result, setResult] = useState<{
    key: string;
    state: PreviewState<T>;
  } | null>(null);
  const latest = useRef(generate);
  useEffect(() => {
    latest.current = generate;
  });
  useEffect(() => {
    if (key === null) return;
    let current = true;
    const settle = (state: PreviewState<T>): void => {
      if (current) setResult({ key, state });
    };
    const timer = window.setTimeout(() => {
      latest.current().then(
        (data) => {
          settle({ status: 'ready', ...data });
        },
        (error: unknown) => {
          settle({ status: 'error', message: message(error) });
        },
      );
    }, SETTLE_MS);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [key]);
  if (key === null) return { status: 'idle' };
  return result?.key === key ? result.state : { status: 'loading' };
}

/** The live DDL of `changes` on `target`; idle while there are none. */
export function useAlterPreview(
  target: TableTargetRef,
  changes: readonly SchemaChangeDto[] | null,
): PreviewState {
  const key =
    changes && changes.length > 0 ? JSON.stringify([target, changes]) : null;
  return usePreview(key, () => tableAlterPreview(target, changes ?? []));
}
