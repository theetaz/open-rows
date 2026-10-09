// SPDX-License-Identifier: Apache-2.0
import type { StatementDto } from '@openrows/ipc';
import type { ExportScope, ExportSheetProps } from './ExportSheet';
import { rowCount } from './exportJob';

type SheetSource = Omit<ExportSheetProps, 'onClose'>;

/**
 * Export a query result from the result store: the received rows, the
 * selected rows when there is a selection, and every row (by running the
 * statement again) when the row limit cut a plain read.
 */
export function resultExport(input: {
  sessionId: string;
  result: { id: string; rows: number; truncated: boolean };
  statement: StatementDto | undefined;
  /** Stored positions of the selected rows, if any. */
  selected?: readonly number[] | undefined;
}): SheetSource {
  const { sessionId, result, statement, selected } = input;
  const rerun =
    result.truncated &&
    statement !== undefined &&
    statement.kind === 'read' &&
    !statement.dangerous;
  const received = result.truncated
    ? `The ${rowCount(result.rows)} received (row limit)`
    : `Every row of the result (${rowCount(result.rows)})`;
  const scopes: ExportScope[] = [];
  if (selected?.length)
    scopes.push({
      label: `The ${rowCount(selected.length)} selected`,
      source: {
        kind: 'rows',
        sessionId,
        resultId: result.id,
        rows: [...selected],
      },
    });
  scopes.push({
    label: received,
    source: { kind: 'result', sessionId, resultId: result.id },
  });
  if (rerun)
    scopes.push({
      label: 'Every row: run the statement again',
      source: { kind: 'query', sessionId, sql: statement.sql },
    });
  return {
    title: 'Export Result',
    fileName: 'result',
    defaultTable: 'result',
    scopes,
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
    scopes: [
      {
        label: 'Every row of the table',
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
      },
    ],
  };
}
