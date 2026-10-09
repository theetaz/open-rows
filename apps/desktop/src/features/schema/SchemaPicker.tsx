// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import type { Catalog } from './useCatalog';
import styles from './SchemaPicker.module.css';

/**
 * Schema of the active database; system schemas only when toggled on.
 * `children` are extra engine tools (e.g. attach for `SQLite`).
 */
export function SchemaPicker({
  catalog,
  children,
}: {
  catalog: Catalog;
  children?: ReactNode;
}): React.JSX.Element {
  const { schemas, systemSchemas } = catalog;
  const names =
    schemas.status === 'ready'
      ? schemas.data
          .filter((schema) => systemSchemas || !schema.system)
          .map((schema) => schema.name)
      : [];
  return (
    <div className={styles.schema}>
      <IconButton aria-label="Create object" size="sm">
        +
      </IconButton>
      <select
        aria-label="Schema"
        value={catalog.schema}
        disabled={!names.length}
        onChange={(event) => {
          catalog.setSchema(event.target.value);
        }}
      >
        {names.map((name) => (
          <option key={name}>{name}</option>
        ))}
      </select>
      <IconButton
        aria-label="Show system schemas"
        aria-pressed={systemSchemas}
        size="sm"
        variant="ghost"
        onClick={() => {
          catalog.setSystemSchemas(!systemSchemas);
        }}
      >
        <Icon name="settings" />
      </IconButton>
      {children}
    </div>
  );
}
