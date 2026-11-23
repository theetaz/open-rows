// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense } from 'react';
import { SplitPanel } from '@openrows/ui';
import type { useWorkspaceController } from './useWorkspaceController';
import { inspectorRange } from './layout';
import type { Connections } from '../connections';
import { QueryContent } from '../query';
import type { ObjectEditorRequest } from '../objects';
const WorkspaceInspector = lazy(() =>
  import('./WorkspaceInspector').then((module) => ({
    default: module.WorkspaceInspector,
  })),
);
import styles from './Frame.module.css';
import { t } from './messages';
type Props = {
  connections: Connections;
  controller: ReturnType<typeof useWorkspaceController>;
};
/** The trigger editor of a table: new (`name: null`), edit or drop. */
function triggerEditor(
  table: { database: string; schema: string; name: string },
  name: string | null,
  drop: boolean,
): ObjectEditorRequest {
  const at = { database: table.database, schema: table.schema };
  return drop && name
    ? {
        kind: 'drop',
        target: { object: 'trigger', ...at, table: table.name, name },
      }
    : { kind: 'trigger', ...at, table: table.name, name };
}

/**
 * Query tabs and their content, with the inspector as a column of its own
 * beside them (only while it has something to show), so it never covers
 * the grid.
 */
export function WorkspaceBody(props: Props): React.JSX.Element {
  const { connections, controller } = props;
  const { tabs, scope } = controller;
  return (
    <div className={styles.body}>
      <QueryContent
        onNewTab={newTabCommand(controller)}
        connections={connections}
        tabs={tabs}
        database={scope?.database ?? ''}
        onCell={controller.onCell}
        onRow={controller.onRow}
        onTableInfo={controller.onTable}
        catalogRevision={controller.catalog.revision}
        onOpenTable={(table) => {
          controller.openObject({ ...table, kind: 'table', arguments: null });
        }}
        onTrigger={(table, name, drop) => {
          controller.objects.open(triggerEditor(table, name, drop));
        }}
        structureRevision={controller.objects.revision}
      />
      <SplitPanel
        label={t('inspector')}
        storageKey="openrows.mock.inspector"
        range={inspectorRange}
        open={controller.inspector.open && hasContent(controller)}
        edge="left"
      >
        <Inspector {...props} />
      </SplitPanel>
    </div>
  );
}

/** The inspector only takes room when it has a selection to show. */
function hasContent(
  controller: ReturnType<typeof useWorkspaceController>,
): boolean {
  return Boolean(controller.cell ?? controller.row ?? controller.table);
}

/** Toolbar and tab strip share the configured new-tab command. */
function newTabCommand(
  controller: ReturnType<typeof useWorkspaceController>,
): () => void {
  return () => {
    controller.command('new-tab');
  };
}

/** Lazy details of the active tab's selection. */
function Inspector({ controller }: Props): React.JSX.Element {
  return (
    <Suspense fallback={null}>
      <WorkspaceInspector
        cell={controller.cell}
        row={controller.row}
        table={controller.table}
      />
    </Suspense>
  );
}
