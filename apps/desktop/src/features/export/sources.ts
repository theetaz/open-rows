// SPDX-License-Identifier: Apache-2.0
import type { StatementDto } from '@openrows/ipc';
import type { ExportSheetProps } from './ExportSheet';

type SheetSource = Omit<ExportSheetProps, 'onClose'>;

/**
 * Export a query result from the result store. When the row limit cut it
 * and its statement is a plain read, the sheet also offers every row by
 * running the statement again.
 */
export function resultExport(input: {
  sessionId: string;
  result: { id: string; rows: number; truncated: boolean };
  statement: StatementDto | undefined;
}): SheetSource {
  const { sessionId, result, statement } = input;
  const rerun =
    result.truncated &&
    statement !== undefined &&
    statement.kind === 'read' &&
    !statement.dangerous;
  return {
    title: 'Export Result',
    fileName: 'result',
    defaultTable: 'result',
    source: { kind: 'result', sessionId, resultId: result.id },
    full: rerun
      ? {
          source: { kind: 'query', sessionId, sql: statement.sql },
          received: result.rows,
        }
      : undefined,
  };
}

/** Export every row of a table or view, streamed from the server. */
export function tableExport(input: {
  sessionId: string;
  profileId: string;
  database: string;
  schema: string;
  name: string;
}): SheetSource {
  return {
    title: `Export ${input.name}`,
    fileName: input.name,
    defaultTable: input.name,
    source: {
      kind: 'table',
      sessionId: input.sessionId,
      target: {
        profileId: input.profileId,
        database: input.database,
        schema: input.schema,
        table: input.name,
      },
    },
  };
}
