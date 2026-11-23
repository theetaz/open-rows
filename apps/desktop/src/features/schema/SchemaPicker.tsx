// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Icon, IconButton } from '@openrows/ui';
import type { Catalog } from './useCatalog';
import styles from './SchemaPicker.module.css';
import { t } from './messages';

type MenuOpener = (point: { x: number; y: number; target?: Element }) => void;

/**
 * The top of the object tree: the database's name with its tools (engine
 * extras in `children`, system schemas, `+` for the create menu), then the
 * schema as a tree row that pops up the schema list (system schemas only
 * when toggled on). Right-click on the schema opens its actions.
 */
export function SchemaPicker({
  catalog,
  onCreate,
  onMenu,
  children,
}: {
  catalog: Catalog;
  onMenu?: MenuOpener | undefined;
  /** Open the create menu (new table, view, schema, …) at a point. */
  onCreate?: ((point: { x: number; y: number }) => void) | undefined;
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
      <div className={styles.head}>
        <span className={styles.database} title={catalog.database}>
          {catalog.database}
        </span>
        <span className={styles.tools}>
          {children}
          <IconButton
            aria-label={t('showSystemSchemas')}
            title={t('showSystemSchemas')}
            aria-pressed={systemSchemas}
            size="sm"
            variant="ghost"
            onClick={() => {
              catalog.setSystemSchemas(!systemSchemas);
            }}
          >
            <Icon name="eye" />
          </IconButton>
          <IconButton
            aria-label={t('createObject')}
            title={t('createObject')}
            aria-haspopup="menu"
            size="sm"
            variant="ghost"
            disabled={!onCreate}
            onClick={(event) => {
              const box = event.currentTarget.getBoundingClientRect();
              onCreate?.({ x: box.left, y: box.bottom + 4 });
            }}
          >
            <Icon name="plus" />
          </IconButton>
        </span>
      </div>
      <SchemaRow catalog={catalog} names={names} onMenu={onMenu} />
    </div>
  );
}

/** The selected schema as a tree row over the native pop-up. */
function SchemaRow({
  catalog,
  names,
  onMenu,
}: {
  catalog: Catalog;
  names: readonly string[];
  onMenu: MenuOpener | undefined;
}): React.JSX.Element {
  return (
    <div
      className={styles.picker}
      onContextMenu={(event) => {
        event.preventDefault();
        onMenu?.({
          x: event.clientX,
          y: event.clientY,
          target: event.currentTarget,
        });
      }}
    >
      <span className={styles.icon} aria-hidden="true">
        {<Icon name={'schemaTree'} />}
      </span>
      <select
        aria-label={t('schema')}
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
      <span className={styles.stepper} aria-hidden="true">
        <Icon name="chevronUpDown" />
      </span>
    </div>
  );
}
