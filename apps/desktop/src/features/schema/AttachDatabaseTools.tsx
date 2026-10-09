// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { Connections } from '../connections';
import { AttachDatabaseSheet } from './AttachDatabaseSheet';
import { useAttachDatabase } from './useAttachDatabase';
import type { Catalog } from './useCatalog';
import styles from './AttachDatabase.module.css';

/**
 * Schema-bar buttons of a `SQLite` connection: "Attach Database…" and, on an
 * attached schema, "Detach"; nothing for other engines.
 */
export function AttachDatabaseTools(props: {
  connections: Connections;
  catalog: Catalog;
}): React.JSX.Element | null {
  const tools = useAttachDatabase(props.connections, props.catalog);
  if (!tools.available) return null;
  return (
    <>
      <IconButton
        aria-label="Attach Database…"
        title="Attach Database…"
        size="sm"
        variant="ghost"
        onClick={tools.choose}
      >
        <Icon name="database" />
      </IconButton>
      {tools.detachable && (
        <IconButton
          aria-label={`Detach ${tools.detachable}`}
          title={`Detach ${tools.detachable}`}
          size="sm"
          variant="ghost"
          onClick={tools.detach}
        >
          <Icon name="close" />
        </IconButton>
      )}
      {tools.notice && (
        <p role="status" className={styles.notice}>
          {tools.notice}
        </p>
      )}
      {tools.pending && (
        <AttachDatabaseSheet
          pending={tools.pending}
          onAlias={tools.setAlias}
          onAttach={tools.confirm}
          onCancel={tools.cancel}
        />
      )}
    </>
  );
}
