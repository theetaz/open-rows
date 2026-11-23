// SPDX-License-Identifier: Apache-2.0
import { useCallback, useState } from 'react';
import { Icon } from '@openrows/ui';
import type { IconName, MenuEntry } from '@openrows/ui';
import { ContextMenu } from '../../shell/ContextMenu';
import type { OpenedDatabase } from './sessions';
import styles from './Rail.module.css';
import { t } from './messages/profile';

type Props = {
  onBackup?: ((name: string) => void) | undefined;
  onRestore?: ((name: string) => void) | undefined;
  databases: readonly OpenedDatabase[];
  /** Active database of the connection. */
  active: string;
  onSelect: (name: string) => void;
  onClose: (name: string) => void;
  /** Open the database's process list; absent when the engine has none. */
  onProcesses?: ((name: string) => void) | undefined;
  /** Open the database Monitor. */
  onMonitor?: ((name: string) => void) | undefined;
  /** Run a `.sql` file on the database. */
  onRunFile?: ((name: string) => void) | undefined;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
};

/** Tooltip: the full name, plus why the database is not usable. */
function tooltip(database: OpenedDatabase): string {
  if (database.state === 'disconnected')
    return t('nameDisconnectedActivate', { name: database.name });
  if (database.state === 'opening')
    return t('nameOpening', { name: database.name });
  if (database.state === 'reconnecting')
    return t('nameReconnectingMessage', {
      name: database.name,
      message: database.message,
    });
  if (database.state === 'idle')
    return t('nameClosedWhile', { name: database.name });
  if (database.state === 'failed')
    return `${database.name} — ${database.message}`;
  return database.name;
}

/** Opened databases of the active connection as rail tiles, like TablePlus. */
export function DatabaseTiles(props: Props): React.JSX.Element | null {
  if (!props.databases.length) return null;
  return (
    <div
      role="group"
      aria-label={t('openedDatabases')}
      className={styles.databases}
    >
      {props.databases.map((database) => (
        <DatabaseTile key={database.name} {...props} database={database} />
      ))}
    </div>
  );
}

function DatabaseTile(
  props: Props & { database: OpenedDatabase },
): React.JSX.Element {
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    target: Element;
  } | null>(null);
  const close = useCallback(() => {
    setMenu(null);
  }, []);
  const { name } = props.database;
  return (
    <>
      <button
        type="button"
        className={styles.database}
        aria-label={t('databaseName', { name })}
        aria-pressed={props.active === name}
        aria-haspopup="menu"
        title={tooltip(props.database)}
        data-state={props.database.state}
        onClick={() => {
          props.onSelect(name);
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          event.currentTarget.focus();
          setMenu({
            x: event.clientX,
            y: event.clientY,
            target: event.currentTarget,
          });
        }}
        onKeyDown={props.onKeyDown}
      >
        <Icon name="database" aria-hidden="true" />
        <span>{name}</span>
      </button>
      {menu && (
        <ContextMenu
          label={t('nameActions', { name })}
          at={menu}
          target={menu.target}
          entries={databaseEntries(props, props.database)}
          onClose={close}
        />
      )}
    </>
  );
}

const item = (
  label: string,
  icon: IconName,
  run: () => void,
  disabled = false,
): MenuEntry => ({
  kind: 'item',
  label,
  icon: <Icon name={icon} />,
  run,
  disabled,
});

/** One optional command: shown only when the rail offers it. */
function optional(
  action: ((name: string) => void) | undefined,
  build: (run: () => void) => MenuEntry,
  name: string,
): MenuEntry[] {
  return action
    ? [
        build(() => {
          action(name);
        }),
      ]
    : [];
}

/** Monitor and processes, back up, restore and run a file, then Close. */
function databaseEntries(props: Props, database: OpenedDatabase): MenuEntry[] {
  const { name } = database;
  const closed = database.state !== 'open';
  const inspect = [
    ...optional(
      props.onMonitor,
      (run) => item(t('showMonitor'), 'activity', run),
      name,
    ),
    ...optional(
      props.onProcesses,
      (run) => item(t('showProcesses'), 'list', run),
      name,
    ),
  ];
  const jobs = [
    ...optional(
      props.onBackup,
      (run) => item(t('backUp'), 'database', run, closed),
      name,
    ),
    ...optional(
      props.onRestore,
      (run) => item(t('restore'), 'history', run, closed),
      name,
    ),
    ...optional(
      props.onRunFile,
      (run) => item(t('runSqlFile'), 'file', run),
      name,
    ),
  ];
  const groups = [inspect, jobs].filter((group) => group.length > 0);
  return [
    ...groups.flatMap((group) => [...group, { kind: 'separator' as const }]),
    item(t('closeDatabase'), 'close', () => {
      props.onClose(name);
    }),
  ];
}
