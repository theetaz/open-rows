// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { resolveQuerySource } from '@openrows/ipc';
import type { EngineDto, SourceResolution } from '@openrows/ipc';
import { resultMapping } from './resultMapping';
import type { ResultMapping } from './resultMapping';
import type { ResultMeta } from './run';

// The result object lives for one execution. Its cache dies when the result is released.
const resolutions = new WeakMap<
  ResultMeta,
  Map<string, Promise<SourceResolution>>
>();
function resolve(
  result: ResultMeta,
  identity: string,
  sessionId: string,
  table: string,
): Promise<SourceResolution> {
  let cache = resolutions.get(result);
  if (!cache) {
    cache = new Map();
    resolutions.set(result, cache);
  }
  let pending = cache.get(identity);
  if (!pending) {
    pending = resolveQuerySource(sessionId, table).catch((error: unknown) => ({
      kind: 'readonly',
      reason: `Could not resolve the source table: ${error instanceof Error ? error.message : String(error)}`,
    }));
    cache.set(identity, pending);
  }
  return pending;
}

/** A new result/run or session always resolves again; stale replies cannot grant editing. */
export function useResultMapping(
  result: ResultMeta,
  sql: string,
  engine: EngineDto,
  sessionId: string | null,
): ResultMapping {
  const mapping = useMemo(
    () => resultMapping(sql, result.columns, engine),
    [sql, result.columns, engine],
  );
  const identity = JSON.stringify([result.id, sql, engine, sessionId]);
  const [state, setState] = useState<{
    identity: string;
    source: SourceResolution;
  } | null>(null);
  useEffect(() => {
    if (mapping.kind !== 'unresolved' || !result.done || !sessionId) return;
    let live = true;
    void resolve(result, identity, sessionId, mapping.table).then((source) => {
      if (live) setState({ identity, source });
    });
    return () => {
      live = false;
    };
  }, [mapping, result, identity, sessionId]);
  return useMemo(() => {
    if (mapping.kind !== 'unresolved' || state?.identity !== identity)
      return mapping;
    return state.source.kind === 'table'
      ? { ...state.source, columns: mapping.columns }
      : state.source;
  }, [mapping, state, identity]);
}
