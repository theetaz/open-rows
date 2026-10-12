// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { useSidebarMenu } from '../../shell/useSidebarMenu';
import type { OpenedDatabase } from './sessions';
import styles from './Rail.module.css';

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
    return `${database.name} — disconnected; activate to connect`;
  if (database.state === 'opening') return `${database.name} — opening…`;
  if (database.state === 'reconnecting')
    return `${database.name} — reconnecting: ${database.message}`;
  if (database.state === 'idle')
    return `${database.name} — closed while idle, reconnects on next query`;
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
      aria-label="Opened databases"
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
  const { opener, id, open, close, toggle } = useSidebarMenu();
  const { name } = props.database;
  return (
    <>
      <button
        ref={opener}
        type="button"
        className={styles.database}
        aria-label={`Database ${name}`}
        aria-pressed={props.active === name}
        aria-haspopup="menu"
        title={tooltip(props.database)}
        data-state={props.database.state}
        onClick={() => {
          props.onSelect(name);
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          toggle();
        }}
        onKeyDown={props.onKeyDown}
      >
        <Icon name="database" aria-hidden="true" />
        <span>{name}</span>
      </button>
      {open && <DatabaseMenu {...props} menu={{ opener, id, close }} />}
    </>
  );
}

function DatabaseMenu(
  props: Props & {
    database: OpenedDatabase;
    menu: Pick<ReturnType<typeof useSidebarMenu>, 'opener' | 'id' | 'close'>;
  },
): React.JSX.Element {
  const { opener, id, close } = props.menu;
  const { name } = props.database;
  return (
    <SidebarMenu
      id={id}
      label={`${name} actions`}
      opener={opener}
      onClose={close}
    >
      {props.onBackup && (
        <button
          type="button"
          role="menuitem"
          disabled={props.database.state !== 'open'}
          onClick={() => {
            close();
            props.onBackup?.(name);
          }}
        >
          Back Up…
        </button>
      )}
      {props.onRestore && (
        <button
          type="button"
          role="menuitem"
          disabled={props.database.state !== 'open'}
          onClick={() => {
            close();
            props.onRestore?.(name);
          }}
        >
          Restore…
        </button>
      )}
      {props.onRunFile && (
        <button
          type="button"
          role="menuitem"
          onClick={() => {
            close();
            props.onRunFile?.(name);
          }}
        >
          Run SQL file…
        </button>
      )}
      <MonitorMenu onMonitor={props.onMonitor} name={name} close={close} />
      {props.onProcesses && (
        <button
          type="button"
          role="menuitem"
          onClick={() => {
            close();
            props.onProcesses?.(name);
          }}
        >
          Show processes
        </button>
      )}
      <button
        type="button"
        role="menuitem"
        onClick={() => {
          close();
          props.onClose(name);
        }}
      >
        Close database
      </button>
    </SidebarMenu>
  );
}

function MonitorMenu(props: {
  onMonitor: Props['onMonitor'];
  name: string;
  close: () => void;
}): React.JSX.Element | null {
  if (!props.onMonitor) return null;
  return (
    <button
      type="button"
      role="menuitem"
      onClick={() => {
        props.close();
        props.onMonitor?.(props.name);
      }}
    >
      Show Monitor
    </button>
  );
}
