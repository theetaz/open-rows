// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button, SegmentedControl } from '@openrows/ui';
import { HistoryRow } from './HistoryRow';
import type { HistoryActions } from './HistoryRow';
import { insertHistorySql } from './historyInsert';
import type { EngineDto, HistoryItemDto } from '@openrows/ipc';
import { useHistory } from './useHistory';
import type { HistoryState } from './useHistory';
import { groupByDay } from './historyView';
import styles from './HistoryList.module.css';

/** A saved connection as the History list names it. */
type HistoryConnection = { id: string; name: string; engine?: EngineDto };
const HistorySaveReview = lazy(() =>
  import('./HistorySaveReview').then((module) => ({
    default: module.HistorySaveReview,
  })),
);

type Scope = 'connection' | 'all';

const scopes: readonly { value: Scope; label: string }[] = [
  { value: 'connection', label: 'This connection' },
  { value: 'all', label: 'All' },
];

type Props = {
  /** The active saved connection, if one is selected. */
  active: HistoryConnection | undefined;
  /** Every saved connection, to name the entries of other connections. */
  connections: readonly HistoryConnection[];
  /** The sidebar's search text: words the statements must contain. */
  text: string;
  /** Open a statement in a new query tab. */
  onOpen: (sql: string) => void;
};

/** Statements run on the active connection (or all), newest first, by day. */
export function HistoryList(props: Props): React.JSX.Element {
  const { active, connections, text, onOpen } = props;
  const [scope, setScope] = useState<Scope>('connection');
  const [saving, setSaving] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const scoped = scope === 'connection';
  const state = useHistory({
    profileId: scoped ? (active?.id ?? null) : null,
    text,
    enabled: !scoped || active !== undefined,
  });
  if (scoped && !active)
    return (
      <p className={styles.empty}>Select a connection to see its history.</p>
    );
  return (
    <div className={styles.history}>
      <HistoryToolbar
        scope={scope}
        onScope={setScope}
        clearable={state.items.length > 0}
        connection={active}
        onClear={state.clear}
      />
      {(state.problem ?? actionError) && (
        <p role="alert">{state.problem ?? actionError}</p>
      )}
      <HistoryDays
        items={state.items}
        text={text}
        connections={connections}
        showConnections={!scoped}
        onOpen={onOpen}
        onInsert={(sql) => {
          setActionError(
            insertHistorySql(sql)
              ? null
              : 'Open a query editor before inserting history.',
          );
        }}
        onSave={(sql) => {
          if (active) setSaving(sql);
          else setActionError('Select a connection before saving history.');
        }}
        state={state}
      />
      {saving !== null && active && (
        <Suspense fallback={null}>
          <HistorySaveReview
            sql={saving}
            profileId={active.id}
            engine={active.engine ?? 'postgres'}
            onClose={() => {
              setSaving(null);
            }}
          />
        </Suspense>
      )}
    </div>
  );
}

/** Scope switch, and the Clear action with its inline confirmation. */
function HistoryToolbar(props: {
  scope: Scope;
  onScope: (scope: Scope) => void;
  clearable: boolean;
  connection: HistoryConnection | undefined;
  onClear: (profileId: string | null) => Promise<void>;
}): React.JSX.Element {
  const [confirming, setConfirming] = useState(false);
  const { connection } = props;
  return (
    <>
      <div className={styles.toolbar}>
        <SegmentedControl
          aria-label="History scope"
          options={scopes}
          value={props.scope}
          onValueChange={(value) => {
            props.onScope(value === 'all' ? 'all' : 'connection');
            setConfirming(false);
          }}
        />
        {props.clearable && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setConfirming(true);
            }}
          >
            Clear
          </Button>
        )}
      </div>
      {confirming && (
        <div role="group" aria-label="Clear history" className={styles.confirm}>
          <p>
            Delete every statement of{' '}
            {props.scope === 'all' ? 'all connections' : connection?.name} from
            History?
          </p>
          <div className={styles.confirmActions}>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => {
                setConfirming(false);
                void props.onClear(
                  props.scope === 'all' ? null : (connection?.id ?? null),
                );
              }}
            >
              Clear history
            </Button>
            <Button
              size="sm"
              onClick={() => {
                setConfirming(false);
              }}
            >
              Cancel
            </Button>
          </div>
        </div>
      )}
    </>
  );
}

/** The entries grouped by day, or the reason there are none. */
function HistoryDays(
  props: HistoryActions & {
    items: readonly HistoryItemDto[];
    text: string;
    connections: readonly HistoryConnection[];
    /** Name the connection of each entry (only when spanning every connection). */
    showConnections: boolean;
    onOpen: (sql: string) => void;
    state: HistoryState;
  },
): React.JSX.Element {
  const groups = groupByDay(props.items, new Date());
  const names = new Map(props.connections.map((item) => [item.id, item.name]));
  if (groups.length === 0)
    return (
      <p role="status" className={styles.empty}>
        {props.text.trim() ? 'No matching statements' : 'No statements run yet'}
      </p>
    );
  return (
    <ol aria-label="Query history" className={styles.days}>
      {groups.map((group) => (
        <li key={group.key}>
          <h3 className={styles.day}>{group.label}</h3>
          <ul>
            {group.items.map((item) => (
              <HistoryRow
                key={item.id}
                item={item}
                connection={
                  props.showConnections ? names.get(item.profileId) : undefined
                }
                onOpen={props.onOpen}
                onInsert={props.onInsert}
                onSave={props.onSave}
                onDelete={props.state.remove}
              />
            ))}
          </ul>
        </li>
      ))}
    </ol>
  );
}
