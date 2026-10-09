// SPDX-License-Identifier: Apache-2.0
import { useEffect, useId, useRef, useState } from 'react';
import { Button, Icon, TextField } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import type { DatabaseDto } from '@openrows/ipc';
import { listIndex, visibleDatabases } from './databaseList';
import type { Catalog } from './useCatalog';
import styles from './OpenDatabasePanel.module.css';

type Props = {
  catalog: Catalog;
  onOpen: (name: string) => void;
  onClose: () => void;
  /** Create a database; absent when the engine cannot. */
  onNew?: (() => void) | undefined;
};

/** Loading, error or offline text while the list is not ready. */
function statusOf(catalog: Catalog, matches: number): string | null {
  if (catalog.offline) return catalog.offline;
  const { databases } = catalog;
  if (databases.status === 'error') return databases.message;
  if (databases.status !== 'ready') return 'Loading databases…';
  return matches ? null : 'No matching databases';
}

/** Focus the panel's search field once the sheet has focused its first control. */
function useSearchFocus(): React.RefObject<HTMLDivElement | null> {
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      panel.current?.querySelector('input')?.focus();
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, []);
  return panel;
}

/**
 * ⌘K "Open database": searchable server databases (system ones on request).
 * ↑/↓ select, Return or double-click opens, Esc cancels.
 */
export function OpenDatabasePanel(props: Props): React.JSX.Element {
  const { catalog } = props;
  const [query, setQuery] = useState('');
  const [system, setSystem] = useState(false);
  const [selected, setSelected] = useState('');
  const panel = useSearchFocus();
  const listId = useId();
  const all =
    catalog.databases.status === 'ready' ? catalog.databases.data : [];
  const shown = visibleDatabases(all, query, system);
  const names = shown.map(({ name }) => name);
  const current = names.includes(selected) ? selected : (names[0] ?? '');
  const open = (): void => {
    if (current) props.onOpen(current);
  };
  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    if (event.nativeEvent.isComposing) return;
    const next = listIndex(event.key, names.indexOf(current), names.length);
    if (event.key !== 'Enter' && next === undefined) return;
    event.preventDefault();
    if (next === undefined) open();
    else setSelected(names[next] ?? '');
  };
  const status = statusOf(catalog, names.length);
  return (
    <Sheet title="Open database" onClose={props.onClose}>
      <div ref={panel} className={styles.panel}>
        <TextField
          aria-label="Search for database"
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-activedescendant={current ? `${listId}-${current}` : undefined}
          placeholder="Search for database…"
          leadingIcon={<Icon name="search" />}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          onKeyDown={onKeyDown}
        />
        <DatabaseList
          id={listId}
          databases={shown}
          opened={catalog.opened}
          current={current}
          onSelect={setSelected}
          onOpen={props.onOpen}
        />
        {status && (
          <p role="status" className={styles.status}>
            {status}
          </p>
        )}
        <PanelFooter
          system={system}
          onSystem={setSystem}
          canOpen={Boolean(current)}
          onOpen={open}
          onCancel={props.onClose}
          onNew={props.onNew}
        />
      </div>
    </Sheet>
  );
}

function DatabaseList(props: {
  id: string;
  databases: readonly DatabaseDto[];
  opened: readonly string[];
  current: string;
  onSelect: (name: string) => void;
  onOpen: (name: string) => void;
}): React.JSX.Element {
  return (
    <ul
      id={props.id}
      role="listbox"
      aria-label="Databases"
      className={styles.list}
    >
      {props.databases.map(({ name, comment }) => (
        <li
          key={name}
          id={`${props.id}-${name}`}
          role="option"
          aria-selected={name === props.current}
          title={comment ?? name}
          className={styles.option}
          ref={(node) => {
            if (
              name === props.current &&
              typeof node?.scrollIntoView === 'function'
            )
              node.scrollIntoView({ block: 'nearest' });
          }}
          onClick={() => {
            props.onSelect(name);
          }}
          onDoubleClick={() => {
            props.onOpen(name);
          }}
        >
          <Icon name="database" aria-hidden="true" />
          <span>{name}</span>
          {props.opened.includes(name) && <small>Open</small>}
        </li>
      ))}
    </ul>
  );
}

function PanelFooter(props: {
  system: boolean;
  onSystem: (show: boolean) => void;
  canOpen: boolean;
  onOpen: () => void;
  onCancel: () => void;
  onNew?: (() => void) | undefined;
}): React.JSX.Element {
  return (
    <>
      <label className={styles.system}>
        <input
          type="checkbox"
          checked={props.system}
          onChange={(event) => {
            props.onSystem(event.target.checked);
          }}
        />
        Show system databases
      </label>
      <footer className={styles.footer}>
        <Button onClick={props.onCancel}>Cancel</Button>
        <span className={styles.spacer} />
        <Button
          disabled={!props.onNew}
          title={
            props.onNew
              ? 'Create a database'
              : 'This database cannot create databases'
          }
          onClick={props.onNew}
        >
          New…
        </Button>
        <Button
          variant="primary"
          disabled={!props.canOpen}
          onClick={props.onOpen}
        >
          Open
        </Button>
      </footer>
    </>
  );
}
