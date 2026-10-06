// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { EnvBadge, Icon } from '@openrows/ui';
import type { Connection } from './data/connections';
import { SidebarMenu } from './SidebarMenu';
import { useSidebarMenu } from './useSidebarMenu';
import styles from './SidebarHeader.module.css';
type DatabaseProps = {
  database: string;
  opened: readonly string[];
  onDatabase: (name: string) => void;
  onCloseDatabase: (name: string) => void;
  onSwitcher: () => void;
};
/** Workspace identity and database navigation above the object browser. */
export function SidebarHeader(
  props: DatabaseProps & { connection: Connection },
): React.JSX.Element {
  return (
    <header className={styles.header}>
      <div className={styles.identity}>
        <strong title={props.connection.name}>{props.connection.name}</strong>
        <EnvBadge environment={props.connection.environment} />
        <ConnectionMenu />
      </div>
      <DatabaseSelector {...props} />
    </header>
  );
}
function ConnectionMenu(): React.JSX.Element {
  const { opener, id, open, close, toggle } = useSidebarMenu();
  const [pressed, setPressed] = useState('');
  return (
    <>
      <button
        type="button"
        className={styles.menuButton}
        ref={opener}
        aria-label="Workspace menu"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-pressed={open}
        onClick={toggle}
      >
        ⋯
      </button>
      {open && (
        <SidebarMenu
          id={id}
          label="Workspace actions"
          opener={opener}
          onClose={close}
        >
          <ConnectionActions pressed={pressed} onPress={setPressed} />
        </SidebarMenu>
      )}
    </>
  );
}
function ConnectionActions(props: {
  pressed: string;
  onPress: (name: string) => void;
}): React.JSX.Element {
  return (
    <>
      {['Connection settings…', 'Duplicate', 'Disconnect'].map((name) => (
        <button
          key={name}
          type="button"
          role="menuitemcheckbox"
          aria-checked={props.pressed === name}
          onClick={() => {
            props.onPress(name);
          }}
        >
          {name}
        </button>
      ))}
    </>
  );
}
function DatabaseSelector(props: DatabaseProps): React.JSX.Element {
  const menu = useSidebarMenu();
  const { opener, id, open, close } = menu;
  return (
    <>
      <DatabaseTrigger {...menu} database={props.database} />
      {open && (
        <SidebarMenu
          id={id}
          label="Opened databases"
          opener={opener}
          onClose={close}
        >
          {props.opened.map((name) => (
            <DatabaseRow key={name} {...props} close={close} name={name} />
          ))}
          <div role="separator" className={styles.divider} />
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              close();
              props.onSwitcher();
            }}
          >
            <span>Open database…</span>
            <kbd>⌘K</kbd>
          </button>
        </SidebarMenu>
      )}
    </>
  );
}
function DatabaseTrigger(
  props: ReturnType<typeof useSidebarMenu> & { database: string },
): React.JSX.Element {
  const { opener, database, open, id, toggle } = props;
  return (
    <button
      ref={opener}
      type="button"
      className={styles.selector}
      aria-label={`Opened databases (${database})`}
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? id : undefined}
      onClick={toggle}
    >
      <Icon name="database" />
      <span>{database}</span>
      <Icon name="chevronDown" />
    </button>
  );
}
function DatabaseRow(
  props: DatabaseProps & { name: string; close: () => void },
): React.JSX.Element {
  const { name, close } = props;
  return (
    <div role="none" className={styles.databaseRow}>
      <button
        type="button"
        role="menuitemradio"
        aria-checked={props.database === name}
        aria-label={`Activate database ${name}`}
        onClick={() => {
          close();
          props.onDatabase(name);
        }}
      >
        <span aria-hidden="true">{props.database === name ? '✓' : ''}</span>
        <span>{name}</span>
      </button>
      <button
        type="button"
        role="menuitem"
        className={styles.close}
        aria-label={`Close database ${name}`}
        disabled={props.opened.length < 2}
        onClick={() => {
          close();
          props.onCloseDatabase(name);
        }}
      >
        ×
      </button>
    </div>
  );
}
