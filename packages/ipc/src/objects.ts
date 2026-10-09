// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  AlterPreviewDto,
  ObjectReply,
  ObjectRequest,
  RoutineTarget,
  SchemaChangeDto,
  TableTarget,
} from './generated/bindings';
import { unwrap } from './connections';

/**
 * The DDL a staged structure change set would run, generated against the
 * catalog without committing anything (the Structure view's live preview).
 */
export function tableAlterPreview(
  target: TableTarget,
  changes: readonly SchemaChangeDto[],
): Promise<AlterPreviewDto> {
  return unwrap(commands.tableAlterPreview(target, [...changes]));
}

/**
 * Review an object lifecycle change (no token: statements and warnings to
 * show), then run it with the review's token and, when asked, the typed name.
 */
export function objectChange(request: ObjectRequest): Promise<ObjectReply> {
  return unwrap(commands.objectChange(request));
}

/** The `CREATE` statement of a function or procedure; `null` when it is gone. */
export function routineDefinition(
  target: RoutineTarget,
): Promise<string | null> {
  return unwrap(commands.routineDefinition(target));
}
