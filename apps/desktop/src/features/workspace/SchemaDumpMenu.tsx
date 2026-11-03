// SPDX-License-Identifier: Apache-2.0
import { PointMenu } from '../../shell/PointMenu';
import type { Catalog, MenuPoint } from '../schema';
import type { JobActions } from './JobActions';
import { t } from './messages';

/** One schema-menu item that closes the menu first. */
function Item(props: {
  label: string;
  disabled?: boolean;
  run: () => void;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <button
      type="button"
      role="menuitem"
      disabled={props.disabled}
      onClick={() => {
        props.onClose();
        props.run();
      }}
    >
      {props.label}
    </button>
  );
}

/** Schema actions reuse the database restore gate and preselect backup objects. */
export function SchemaDumpMenu(props: {
  schema: string;
  point: MenuPoint;
  catalog: Catalog;
  jobs: JobActions;
  /** Copy the schema's `openrows://` link; omitted without a saved connection. */
  onCopyLink?: ((schema: string) => void) | undefined;
  onClose: () => void;
}): React.JSX.Element {
  const { catalog, jobs, schema, onClose } = props;
  const database = catalog.database;
  return (
    <PointMenu
      {...props.point}
      label={t('schemaActions', { schema })}
      onClose={onClose}
    >
      <Item
        label={t('backUp')}
        disabled={!!catalog.offline}
        onClose={onClose}
        run={() => {
          jobs.backup(database, { schemas: [schema], tables: [] });
        }}
      />
      <Item
        label={t('restore')}
        disabled={!!catalog.offline}
        onClose={onClose}
        run={() => {
          jobs.restore(
            database,
            catalog.engine === 'postgres' ? schema : undefined,
          );
        }}
      />
      {props.onCopyLink && (
        <Item
          label={t('copyLink')}
          onClose={onClose}
          run={() => {
            props.onCopyLink?.(schema);
          }}
        />
      )}
    </PointMenu>
  );
}
