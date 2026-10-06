// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton, useReducedMotion } from '@openrows/ui';
import { connections } from './data/connections';
import type { Database } from './data/databases';
import { databases } from './data/databases';
import styles from './Workspace.module.css';

/** Fast connection switches and the retained design gallery entry point. */
export function ConnectionRail({
  selected,
  onSelect,
  onGallery,
  opened,
  database,
  onDatabase,
  onCloseDatabase,
}: {
  selected: number;
  onSelect: (index: number) => void;
  onGallery: () => void;
  opened: readonly string[];
  database: string;
  onDatabase: (name: string) => void;
  onCloseDatabase: (name: string) => void;
}): React.JSX.Element {
  const reduced = useReducedMotion();
  return (
    <nav className={styles.rail} aria-label="Workspaces">
      {connections.map((connection, index) => (
        <div key={connection.name} className={styles.workspaceTile}>
          <button
            type="button"
            key={connection.name}
            className={styles.connection}
            aria-label={`Switch to ${connection.name}`}
            aria-pressed={selected === index}
            onClick={() => {
              onSelect(index);
            }}
          >
            <span
              className={styles.tile}
              data-environment={connection.environment}
            >
              {connection.initials}
              <i aria-hidden="true" />
            </span>
            <small>{connection.short}</small>
          </button>
          <RailDatabases
            selected={selected === index}
            opened={opened}
            database={database}
            catalog={databases[index] ?? []}
            onDatabase={onDatabase}
            onCloseDatabase={onCloseDatabase}
            reduced={reduced}
          />
        </div>
      ))}
      <IconButton
        className={styles.addConnection}
        aria-label="New connection"
        variant="ghost"
      >
        <Icon name="plus" />
      </IconButton>
      <div className={styles.railSpace} />
      <Button aria-label="API client" size="sm">
        API
      </Button>
      <IconButton
        aria-label="Component gallery"
        variant="ghost"
        onClick={onGallery}
      >
        <Icon name="settings" />
      </IconButton>
    </nav>
  );
}

function RailDatabases({
  selected,
  opened,
  database,
  catalog,
  onDatabase,
  onCloseDatabase,
  reduced,
}: {
  selected: boolean;
  opened: readonly string[];
  database: string;
  catalog: readonly Database[];
  onDatabase: (name: string) => void;
  onCloseDatabase: (name: string) => void;
  reduced: boolean;
}): React.JSX.Element {
  return (
    <>
      {catalog.map((item) => {
        const open = selected && opened.includes(item.name);
        return (
          <div
            key={item.name}
            className={styles.databaseReveal}
            data-open={open}
            data-motion={reduced ? 'reduced' : 'full'}
            inert={!open}
            aria-hidden={!open}
          >
            <div>
              <div className={styles.databaseRow}>
                <button
                  type="button"
                  title={item.name}
                  aria-label={`Activate database ${item.name}`}
                  aria-pressed={database === item.name}
                  onClick={() => {
                    onDatabase(item.name);
                  }}
                >
                  <Icon name="database" width={16} height={16} />
                  <span>{item.name}</span>
                </button>
                <button
                  type="button"
                  aria-label={`Close database ${item.name}`}
                  disabled={opened.length < 2}
                  onClick={() => {
                    onCloseDatabase(item.name);
                  }}
                >
                  ×
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </>
  );
}
