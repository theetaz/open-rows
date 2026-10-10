// SPDX-License-Identifier: Apache-2.0
import { PointMenu } from '../../shell/PointMenu';
import type { Catalog, MenuPoint } from '../schema';
import type { JobActions } from './JobActions';

/** Schema actions reuse the database restore gate and preselect backup objects. */
export function SchemaDumpMenu(props: {
  schema: string;
  point: MenuPoint;
  catalog: Catalog;
  jobs: JobActions;
  onClose: () => void;
}): React.JSX.Element {
  const database = props.catalog.database;
  return (
    <PointMenu
      {...props.point}
      label={`${props.schema} actions`}
      onClose={props.onClose}
    >
      <button
        type="button"
        role="menuitem"
        disabled={!!props.catalog.offline}
        onClick={() => {
          props.onClose();
          props.jobs.backup(database, { schemas: [props.schema], tables: [] });
        }}
      >
        Back Up…
      </button>
      <button
        type="button"
        role="menuitem"
        disabled={!!props.catalog.offline}
        onClick={() => {
          props.onClose();
          props.jobs.restore(
            database,
            props.catalog.engine === 'postgres' ? props.schema : undefined,
          );
        }}
      >
        Restore…
      </button>
    </PointMenu>
  );
}
