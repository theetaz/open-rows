// SPDX-License-Identifier: Apache-2.0
import { Icon } from '@openrows/ui';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { useSidebarMenu } from '../../shell/useSidebarMenu';
import styles from './DatabaseSelector.module.css';

type Props = {
  /** Active database; empty when nothing is connected. */
  database: string;
  opened: readonly string[];
  onDatabase: (name: string) => void;
  onCloseDatabase: (name: string) => void;
  onSwitcher: () => void;
};

/** Opened databases of the active connection, with ⌘K to open another. */
export function DatabaseSelector(props: Props): React.JSX.Element {
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
      aria-label={
        database ? `Opened databases (${database})` : 'Opened databases'
      }
      aria-haspopup="menu"
      aria-expanded={open}
      aria-controls={open ? id : undefined}
      disabled={!database}
      onClick={toggle}
    >
      <Icon name="database" />
      <span>{database || 'No database'}</span>
      <Icon name="chevronDown" />
    </button>
  );
}

function DatabaseRow(
  props: Props & { name: string; close: () => void },
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
