// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, IconButton } from '@openrows/ui';
import { connections } from './data/connections';
import styles from './Workspace.module.css';

/** Fast connection switches and the retained design gallery entry point. */
export function ConnectionRail({
  selected,
  onSelect,
  onGallery,
}: {
  selected: number;
  onSelect: (index: number) => void;
  onGallery: () => void;
}): React.JSX.Element {
  return (
    <nav className={styles.rail} aria-label="Connections">
      {connections.map((connection, index) => (
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
