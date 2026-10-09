// SPDX-License-Identifier: Apache-2.0
import type { RoutineKindDto } from '@openrows/ipc';

/** An object's database and schema. */
type Where = { database: string; schema: string };

/** What a drop (or truncate) removes. */
export type DropTarget =
  | ({ object: 'table' | 'view' | 'materializedView'; name: string } & Where)
  | ({ object: 'trigger'; table: string; name: string } & Where)
  | ({ object: RoutineKindDto; name: string; arguments: string | null } & Where)
  | { object: 'database'; database: string; name: string }
  | { object: 'schema'; database: string; name: string };

/** An object editor to open; `name: null` creates a new object. */
export type ObjectEditorRequest =
  | ({ kind: 'newTable' } & Where)
  | ({
      kind: 'renameTable' | 'duplicateTable' | 'truncateTable';
      name: string;
    } & Where)
  | ({ kind: 'view'; name: string | null; materialized: boolean } & Where)
  | ({ kind: 'trigger'; table: string; name: string | null } & Where)
  | ({
      kind: 'routine';
      routine: RoutineKindDto;
      name: string | null;
      arguments: string | null;
    } & Where)
  | { kind: 'database' | 'schema'; database: string; name: string | null }
  | { kind: 'drop'; target: DropTarget };

/** What the object kind of a drop is called in titles and messages. */
export const objectNoun: Record<DropTarget['object'], string> = {
  table: 'table',
  view: 'view',
  materializedView: 'materialized view',
  trigger: 'trigger',
  function: 'function',
  procedure: 'procedure',
  database: 'database',
  schema: 'schema',
};
