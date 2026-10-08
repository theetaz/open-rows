// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { EditToolbar } from '../EditToolbar';
import type { EditBarModel } from '../EditToolbar';
import { ColumnEditor } from './ColumnEditor';
import { DdlPopover } from './DdlPopover';
import { IndexEditor } from './IndexEditor';
import { IndexSheet } from './IndexSheet';
import { ItemMenu } from './ItemMenu';
import { SchemaReview } from './SchemaReview';
import { describe } from './schemaChanges';
import { StructureHeader } from './StructureHeader';
import { TriggersSheet } from './TriggersSheet';
import type { SchemaEditing } from './useSchemaEditing';
import type { Structure } from './useStructure';
import tableStyles from '../Table.module.css';
import styles from './Structure.module.css';

/** The pending-changes bar model of staged structure changes. */
function schemaEditBar(schema: SchemaEditing): EditBarModel {
  const { history } = schema;
  const count = history.current.length;
  return {
    count,
    note: history.uncertain
      ? 'Commit status unknown; reload before editing.'
      : `${String(count)} staged structure ${count === 1 ? 'change' : 'changes'}`,
    canUndo: history.past.length > 0,
    canRedo: history.future.length > 0,
    uncertain: Boolean(history.uncertain),
    busy: schema.busy,
    enabled: schema.enabled,
    undo: schema.undo,
    redo: schema.redo,
    discard: schema.discard,
    commit: () => {
      schema.commit();
    },
    list: (id) => (
      <div
        id={id}
        className={tableStyles.stagedRows}
        aria-label="Staged changes"
      >
        {history.current.map((item) => (
          <div key={item.id}>
            <span>{describe(item.change)}</span>
            <Button
              size="sm"
              disabled={schema.busy}
              onClick={() => {
                schema.unstage(item.id);
              }}
            >
              Remove
            </Button>
          </div>
        ))}
      </div>
    ),
  };
}

/** Structure or Index mode: header, grid, menus, sheets and the review. */
export function StructurePane({
  structure,
  table,
}: {
  structure: Structure;
  table: string;
}): React.JSX.Element {
  const { detail, schema } = structure;
  if (detail.status === 'error')
    return (
      <p role="alert" className={styles.error}>
        {detail.message}
      </p>
    );
  if (detail.status !== 'ready')
    return (
      <p role="status" className={styles.note}>
        Loading structure…
      </p>
    );
  return (
    <div className={styles.pane}>
      <StructureHeader structure={structure} table={table} />
      {structure.mode === 'indexes' ? (
        <IndexEditor structure={structure} />
      ) : (
        <ColumnEditor structure={structure} />
      )}
      {schema.error && !schema.review && (
        <p role="alert" className={tableStyles.banner}>
          {schema.error}
        </p>
      )}
      <EditToolbar model={schemaEditBar(schema)} />
      <Overlays structure={structure} table={table} />
    </div>
  );
}

function Overlays({
  structure,
  table,
}: {
  structure: Structure;
  table: string;
}): React.JSX.Element {
  const { schema } = structure;
  const close = (): void => {
    structure.setSheet(null);
  };
  return (
    <>
      {structure.menu && (
        <ItemMenu
          menu={structure.menu}
          onClose={() => {
            structure.setMenu(null);
          }}
        />
      )}
      {structure.sheet === 'index' && (
        <IndexSheet
          columns={structure.rows
            .filter((row) => !row.dropped)
            .map((row) => row.name)}
          onStage={schema.stage}
          onClose={close}
        />
      )}
      {structure.sheet === 'triggers' && schema.detail && (
        <TriggersSheet
          table={table}
          triggers={schema.detail.triggers}
          onClose={close}
        />
      )}
      {structure.ddl && schema.detail && (
        <DdlPopover
          detail={schema.detail}
          anchor={structure.ddl}
          onClose={structure.closeDdl}
        />
      )}
      <SchemaReview key={schema.review?.token} schema={schema} />
    </>
  );
}
