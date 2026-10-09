// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, IconButton, SegmentedControl } from '@openrows/ui';
import type { HistoryItemDto } from '@openrows/ipc';
import { useHistory } from './useHistory';
import type { HistoryState } from './useHistory';
import { groupByDay, summaryOf } from './historyView';
import styles from './HistoryList.module.css';

/** A saved connection as the History list names it. */
type HistoryConnection = { id: string; name: string };

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
        clearable={scoped && active !== undefined && state.items.length > 0}
        connection={active}
        onClear={state.clear}
      />
      {state.problem && <p role="alert">{state.problem}</p>}
      <HistoryDays
        items={state.items}
        text={text}
        connections={connections}
        showConnections={!scoped}
        onOpen={onOpen}
        state={state}
      />
    </div>
  );
}

/** Scope switch, and the Clear action with its inline confirmation. */
function HistoryToolbar(props: {
  scope: Scope;
  onScope: (scope: Scope) => void;
  clearable: boolean;
  connection: HistoryConnection | undefined;
  onClear: (profileId: string) => Promise<void>;
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
      {confirming && connection && (
        <div role="group" aria-label="Clear history" className={styles.confirm}>
          <p>Delete every statement of {connection.name} from History?</p>
          <div className={styles.confirmActions}>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => {
                setConfirming(false);
                void props.onClear(connection.id);
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
function HistoryDays(props: {
  items: readonly HistoryItemDto[];
  text: string;
  connections: readonly HistoryConnection[];
  /** Name the connection of each entry (only when spanning every connection). */
  showConnections: boolean;
  onOpen: (sql: string) => void;
  state: HistoryState;
}): React.JSX.Element {
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
                onDelete={props.state.remove}
              />
            ))}
          </ul>
        </li>
      ))}
    </ol>
  );
}

/** One statement: a button that opens it, and copy and delete actions. */
function HistoryRow(props: {
  item: HistoryItemDto;
  /** Connection name, shown when the list spans every connection. */
  connection: string | undefined;
  onOpen: (sql: string) => void;
  onDelete: (id: number) => Promise<void>;
}): React.JSX.Element {
  const { item } = props;
  const { id } = item;
  const label = snippet(item.statement);
  const meta = [props.connection, item.database, ...summaryOf(item)];
  return (
    <li className={styles.row} data-status={item.status}>
      <button
        type="button"
        className={styles.open}
        title={item.errorMessage ?? item.statement}
        onClick={() => {
          props.onOpen(item.statement);
        }}
      >
        <code>{label}</code>
        <small>{meta.filter(Boolean).join(' · ')}</small>
      </button>
      <span className={styles.actions}>
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={`Copy ${label}`}
          onClick={() => {
            void navigator.clipboard
              .writeText(item.statement)
              .catch(() => undefined);
          }}
        >
          <Icon name="copy" />
        </IconButton>
        {id !== null && (
          <IconButton
            size="sm"
            variant="ghost"
            aria-label={`Delete ${label}`}
            onClick={() => {
              void props.onDelete(id);
            }}
          >
            <Icon name="close" />
          </IconButton>
        )}
      </span>
    </li>
  );
}

/** The statement on one line, cut for labels. */
function snippet(statement: string): string {
  const flat = statement.replace(/\s+/g, ' ').trim();
  return flat.length > 60 ? `${flat.slice(0, 59)}…` : flat;
}
