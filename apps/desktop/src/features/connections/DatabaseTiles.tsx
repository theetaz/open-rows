// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { useSidebarMenu } from '../../shell/useSidebarMenu';
import type { OpenedDatabase } from './sessions';
import styles from './Rail.module.css';

type Props = {
  databases: readonly OpenedDatabase[];
  /** Active database of the connection. */
  active: string;
  onSelect: (name: string) => void;
  onClose: (name: string) => void;
  onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => void;
};

/** Tooltip: the full name, plus why the database is not usable. */
function tooltip(database: OpenedDatabase): string {
  if (database.state === 'opening') return `${database.name} — opening…`;
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
      {open && (
        <SidebarMenu
          id={id}
          label={`${name} actions`}
          opener={opener}
          onClose={close}
        >
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
      )}
    </>
  );
}
