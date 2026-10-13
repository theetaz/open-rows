// SPDX-License-Identifier: Apache-2.0
import { useEffect, useMemo, useState } from 'react';
import { tableDetail, tableEditInfo } from '@openrows/ipc';
import type { EditInfoDto, EngineDto } from '@openrows/ipc';
import type { RowEditing } from '../table/useRowEditing';
import type { ResultWorkspace } from './ResultWorkspace';
import type { TableTargetRef } from '../table/model';
import { useRowEditing } from '../table/useRowEditing';
import { useTableViews } from '../table/useTableViews';
import { useResultWorkspace } from './ResultWorkspace';
import { mappingForBatch, mappingReason } from './resultMapping';
import type { ResultMapping } from './resultMapping';
import { useResultMapping } from './useResultMapping';
import type { ResultMeta } from './run';

/** Catalog lookups never grant editing to a stale SQL/result/session identity. */
function useMappingInfo(args: {
  target: Parameters<typeof tableEditInfo>[0];
  enabled: boolean;
  identity: string;
}): { info: EditInfoDto | null; error: string | null } {
  const [state, setState] = useState<{
    identity: string;
    info: EditInfoDto | null;
    error: string | null;
  } | null>(null);
  const { target, enabled, identity } = args;
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    void Promise.all([tableEditInfo(target), tableDetail(target)]).then(
      ([info, detail]) => {
        if (live)
          setState({
            identity,
            info: detail.info.kind === 'table' ? info : null,
            error:
              detail.info.kind === 'table'
                ? null
                : 'Only base tables can be edited from query results.',
          });
      },
      (error: unknown) => {
        if (live)
          setState({
            identity,
            info: null,
            error: `Could not verify the source table: ${error instanceof Error ? error.message : String(error)}`,
          });
      },
    );
    return () => {
      live = false;
    };
  }, [target, enabled, identity]);
  return enabled && state?.identity === identity
    ? state
    : { info: null, error: null };
}

type ResultEditing = {
  editing: RowEditing;
  reason: string | null;
  target: TableTargetRef;
  workspace: ResultWorkspace | null;
  ready: boolean;
  mapped: boolean;
};

/** Use the table's exact snapshots, staging queue and transactional commit flow. */
export function useResultEditing(
  result: ResultMeta,
  sql: string,
  engine: EngineDto,
  statementCount = 1,
): ResultEditing {
  const workspace = useResultWorkspace();
  const localViews = useTableViews();
  const views = workspace?.views ?? localViews;
  const sessionId = workspace?.sessionId ?? null;
  const source = useResultMapping(
    result,
    sql,
    engine,
    statementCount === 1 ? sessionId : null,
  );
  const mapping = useMemo(
    () => mappingForBatch(source, statementCount),
    [source, statementCount],
  );
  const target = useMemo(
    () => ({
      profileId: workspace?.profileId ?? '',
      database: workspace?.database ?? '',
      schema: mapping.kind === 'table' ? mapping.schema : '',
      table: mapping.kind === 'table' ? mapping.table : '',
    }),
    [workspace?.profileId, workspace?.database, mapping],
  );
  const tabId = `${workspace?.tabId ?? 'preview'}:result:${result.id}`;
  const revision = views.entryOf(tabId).view.revision;
  const state = useMappingInfo({
    target,
    enabled: !!sessionId && result.done && mapping.kind === 'table',
    identity: JSON.stringify([result.id, sql, target, sessionId]),
  });
  const reason = readOnlyReason({
    revision,
    done: result.done,
    sessionId,
    mapping,
    ...state,
  });
  const info = useMemo(
    () =>
      state.info && reason ? { ...state.info, readOnly: true } : state.info,
    [state.info, reason],
  );
  const editing = useRowEditing({
    tabId,
    target,
    sessionId,
    views,
    offset: 0,
    info,
  });
  return {
    editing,
    reason,
    target,
    workspace,
    ready: reason === null,
    mapped: mapping.kind === 'table',
  };
}

function readOnlyReason(args: {
  revision: number;
  done: boolean;
  sessionId: string | null;
  mapping: ResultMapping;
  info: EditInfoDto | null;
  error: string | null;
}): string | null {
  if (args.revision > 0)
    return 'The stored result is out of date after a commit. Run the SELECT again before editing.';
  if (!args.done) return 'Wait until every row has arrived before editing.';
  if (!args.sessionId)
    return 'Connect this query tab before editing its results.';
  return args.error ?? mappingReason(args.mapping, args.info);
}
