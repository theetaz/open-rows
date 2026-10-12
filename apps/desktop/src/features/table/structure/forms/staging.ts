// SPDX-License-Identifier: Apache-2.0
import type { SchemaChangeDto } from '@openrows/ipc';
import { replaceStaged, stageSchema, unstage } from '../schemaChanges';
import type { SchemaBase, SchemaHistory } from '../schemaChanges';
import type { SchemaEditing } from '../useSchemaEditing';

/** What a structure form stages: new changes, or a staged change replaced. */
export type StageOp =
  | {
      kind: 'stage';
      changes: readonly SchemaChangeDto[];
      remove?: readonly string[];
    }
  | { kind: 'replace'; id: string; change: SchemaChangeDto };

/** The staged history after `op`, without staging anything (for previews). */
function withStaged(
  history: SchemaHistory,
  op: StageOp,
  base: SchemaBase,
): SchemaHistory {
  if (op.kind === 'replace') return replaceStaged(history, op.id, op.change);
  const remaining = (op.remove ?? []).reduce(unstage, history);
  return op.changes.reduce(
    (current, change) => stageSchema(current, change, base),
    remaining,
  );
}

/** The staged changes after `op`, for the live preview. */
export function plannedChanges(
  schema: SchemaEditing,
  op: StageOp | null,
  table: string,
): SchemaChangeDto[] | null {
  if (!op || !schema.detail) return null;
  const history = withStaged(schema.history, op, {
    detail: schema.detail,
    table,
  });
  return history.current.map((item) => item.change);
}

/** Stage `op` for real. */
export function applyOp(schema: SchemaEditing, op: StageOp): void {
  if (op.kind === 'replace') schema.replace(op.id, op.change);
  else {
    for (const id of op.remove ?? []) schema.unstage(id);
    for (const change of op.changes) schema.stage(change);
  }
}
