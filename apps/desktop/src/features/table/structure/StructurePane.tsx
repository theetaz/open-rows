// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { EditToolbar } from '../EditToolbar';
import type { EditBarModel } from '../EditToolbar';
import { KeySheet } from './KeySheet';
import { CheckSheet } from './CheckSheet';
import { ColumnEditor } from './ColumnEditor';
import { ColumnSheet } from './ColumnSheet';
import { ConstraintsSheet } from './ConstraintsSheet';
import { ForeignKeySheet } from './ForeignKeySheet';
import { DdlPopover } from './DdlPopover';
import { IndexEditor } from './IndexEditor';
import { IndexSheet } from './IndexSheet';
import { ItemMenu } from './ItemMenu';
import { PartitionList } from './PartitionList';
import { SchemaReview } from './SchemaReview';
import { StagedPreview } from './StagedPreview';
import { describe } from './schemaChanges';
import { StructureHeader } from './StructureHeader';
import { TriggersSheet } from './TriggersSheet';
import type { Structure } from './useStructure';
import tableStyles from '../Table.module.css';
import styles from './Structure.module.css';

/** The pending-changes bar model of staged structure changes. */
function schemaEditBar(structure: Structure): EditBarModel {
  const { schema } = structure;
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
        <StagedPreview structure={structure} />
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
      {structure.mode === 'structure' && !!schema.detail?.partitions.length && (
        <PartitionList partitions={schema.detail.partitions} />
      )}
      {schema.error && !schema.review && (
        <p role="alert" className={tableStyles.banner}>
          {schema.error}
        </p>
      )}
      <EditToolbar model={schemaEditBar(structure)} />
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
      <FormSheets structure={structure} onClose={close} />
      {structure.sheet === 'triggers' && schema.detail && (
        <TriggersSheet
          table={table}
          triggers={schema.detail.triggers}
          onTrigger={structure.onTrigger}
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

/** The structure forms: column, index, constraints, foreign key and check. */
function FormSheets({
  structure,
  onClose,
}: {
  structure: Structure;
  onClose: () => void;
}): React.JSX.Element | null {
  switch (structure.sheet) {
    case 'column':
      return (
        <ColumnSheet
          key={structure.columnRow?.id ?? 'new'}
          structure={structure}
          row={structure.columnRow}
          onClose={onClose}
        />
      );
    case 'index':
      return <IndexSheet structure={structure} onClose={onClose} />;
    case 'constraints':
      return <ConstraintsSheet structure={structure} onClose={onClose} />;
    case 'foreignKey':
      return <ForeignKeySheet structure={structure} onClose={onClose} />;
    case 'key':
      return <KeySheet structure={structure} onClose={onClose} />;
    case 'check':
      return <CheckSheet structure={structure} onClose={onClose} />;
    default:
      return null;
  }
}
