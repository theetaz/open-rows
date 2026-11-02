// SPDX-License-Identifier: Apache-2.0
import { restoreTabs, serializeTabs } from '../query/tabStorage';
import { emptyTabs } from '../query/tabs';
import type { QueryTab, TabScope, TabsState } from '../query/tabs';
import type { TableEntry } from '../table/entries';

/** One-scope utility tabs (processes, Monitor) that session storage skips. */
type Utility = TabScope & { kind: 'processes' | 'monitor' };

/** Tabs on their way to another window, with the scope to show there. */
export type Transfer = {
  tabs: TabsState;
  utilities: readonly Utility[];
  /** The moved tab's connection and database, selected in the target window. */
  scope: TabScope | null;
};

const isUtility = (tab: QueryTab): boolean =>
  tab.kind === 'processes' || tab.kind === 'monitor';

/**
 * Serialise tabs for another window: the allowlisted session descriptors,
 * always with query text (a move is not a relaunch), and never results.
 */
export function transferPayload(tabs: readonly QueryTab[]): string {
  const moved: TabsState = { ...emptyTabs, tabs };
  const utilities = tabs
    .filter(isUtility)
    .map(({ kind, connectionId, database }): Utility => ({
      kind: kind as Utility['kind'],
      connectionId,
      database,
    }));
  const first = tabs[0];
  return JSON.stringify({
    version: 1,
    tabs: serializeTabs(moved, true),
    utilities,
    scope: first
      ? { connectionId: first.connectionId, database: first.database }
      : null,
  });
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

function scopeOf(value: unknown): TabScope | null {
  if (!isRecord(value)) return null;
  const { connectionId, database } = value;
  return typeof connectionId === 'string' && typeof database === 'string'
    ? { connectionId, database }
    : null;
}

function utilityOf(value: unknown): Utility | null {
  const scope = scopeOf(value);
  if (!scope || !isRecord(value)) return null;
  return value.kind === 'processes' || value.kind === 'monitor'
    ? { ...scope, kind: value.kind }
    : null;
}

/** Read a payload another window sent; `null` when it is unusable. */
export function parseTransfer(payload: string): Transfer | null {
  let value: unknown;
  try {
    value = JSON.parse(payload);
  } catch {
    return null;
  }
  if (!isRecord(value) || value.version !== 1) return null;
  if (typeof value.tabs !== 'string' || !Array.isArray(value.utilities))
    return null;
  return {
    tabs: restoreTabs(value.tabs),
    utilities: value.utilities
      .map(utilityOf)
      .filter((item): item is Utility => item !== null),
    scope: scopeOf(value.scope),
  };
}

/** What moving a tab does to its uncommitted work. */
export type MoveImpact = {
  /** A commit is running: the tab must stay until it finishes. */
  pending: boolean;
  /** Staged table-row edits, kept in the local journal and offered for restore. */
  journaled: number;
  /** Staged query-result edits and structure changes, which do not move. */
  lost: number;
};

/** The impact of moving `tab`, whose table or result state is `entry`. */
export function moveImpact(
  tab: QueryTab,
  entry: Pick<TableEntry, 'edits' | 'schema'>,
): MoveImpact {
  const { edits, schema } = entry;
  const rows = edits.current.length;
  return {
    pending: edits.pending || schema.pending,
    journaled: tab.kind === 'table' ? rows : 0,
    lost: schema.current.length + (tab.kind === 'table' ? 0 : rows),
  };
}
