// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { tableDetail } from '@openrows/ipc';
import type { ObjectChangeDto, ObjectFeaturesDto } from '@openrows/ipc';
import type { Connections } from '../connections';
import { objectCommandRequest, objectCommandSupported } from '../objects';
import type {
  CommandContext,
  ObjectCommand,
  ObjectEditorRequest,
} from '../objects';
import type { QueryTabs, TabScope } from '../query';
import type { Catalog, ObjectRef } from '../schema';

type Input = {
  connections: Connections;
  catalog: Catalog;
  tabs: QueryTabs;
  scope: TabScope | undefined;
  scopeOf: (database: string) => TabScope | undefined;
  openObject: (object: ObjectRef) => void;
};

/** The open object editor and how it opens, closes and finishes. */
export type ObjectEditing = {
  editor: ObjectEditorRequest | null;
  /** The active connection's editors; `null` without a connection. */
  features: ObjectFeaturesDto | null;
  open: (request: ObjectEditorRequest) => void;
  close: () => void;
  /** After a change ran: refresh the catalog and keep tabs in step. */
  done: (request: ObjectEditorRequest, change: ObjectChangeDto) => void;
  /** Run an object command from the palette or menu bar. */
  command: (id: ObjectCommand) => void;
  /** Counts changes made through the editors; table tabs re-read on it. */
  revision: number;
};

/** The browsed schema and the active table tab's relation. */
function commandContext(input: Input): CommandContext {
  const { catalog, tabs, scope } = input;
  const tab = scope ? tabs.activeFor(scope) : undefined;
  return {
    place: catalog.database
      ? { database: catalog.database, schema: catalog.schema }
      : null,
    table: tab?.kind === 'table' && tab.table ? tab.table : null,
  };
}

/** Tabs of the relation `target` names in `database`. */
function relationTabs(
  input: Input,
  database: string,
  target: { schema: string; name: string },
): string[] {
  const scope = input.scopeOf(database);
  if (!scope) return [];
  return input.tabs
    .tabsFor(scope)
    .filter(
      ({ table }) =>
        table?.schema === target.schema && table.name === target.name,
    )
    .map((tab) => tab.id);
}

/** Rename or close the tabs of a renamed or dropped relation; open a new table. */
function followUp(
  input: Input,
  database: string,
  change: ObjectChangeDto,
): void {
  switch (change.kind) {
    case 'renameTable':
      for (const id of relationTabs(input, database, change.target))
        input.tabs.renameTable?.(id, change.name);
      return;
    case 'dropTable':
    case 'dropView':
      for (const id of relationTabs(input, database, change.target))
        input.tabs.close(id, { discard: true });
      return;
    case 'createTable':
      input.openObject({
        database,
        ...change.target,
        kind: 'table',
        arguments: null,
      });
      return;
    default:
  }
}

/** A drop of the active tab's view, as a view or a materialized view. */
async function resolvedDrop(
  profileId: string,
  request: ObjectEditorRequest,
): Promise<ObjectEditorRequest> {
  if (request.kind !== 'drop' || request.target.object !== 'view')
    return request;
  const { target } = request;
  const detail = await tableDetail({
    profileId,
    database: target.database,
    schema: target.schema,
    table: target.name,
  });
  const object =
    detail.info.kind === 'materializedView' ? 'materializedView' : 'view';
  return { kind: 'drop', target: { ...target, object } };
}

/** Object editors of the workspace (M2-08). */
export function useObjectEditing(input: Input): ObjectEditing {
  const [editor, setEditor] = useState<ObjectEditorRequest | null>(null);
  const [revision, setRevision] = useState(0);
  const profile = input.connections.active;
  const features = profile?.objects ?? null;
  return {
    editor,
    features,
    open: setEditor,
    close: () => {
      setEditor(null);
    },
    revision,
    done: (request, change) => {
      setEditor(null);
      setRevision((count) => count + 1);
      input.catalog.refresh();
      const database =
        'database' in request ? request.database : request.target.database;
      followUp(input, database, change);
    },
    command: (id) => {
      if (!profile || !features || !objectCommandSupported(id, features))
        return;
      const request = objectCommandRequest(id, commandContext(input));
      if (!request) return;
      resolvedDrop(profile.id, request).then(setEditor, () => {
        setEditor(request);
      });
    },
  };
}
