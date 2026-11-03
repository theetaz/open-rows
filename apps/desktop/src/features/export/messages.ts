// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t, tn } = defineMessages({
  cancelledNoFile: 'Cancelled. No file was written.',
  exportFilename: 'Export {fileName}',
  rowsReceivedRow: 'The {rows} received (row limit)',
  everyRowResult: 'Every row of the result ({rows})',
  countSelected: 'The {count} selected',
  everyRowRun: 'Every row: run the statement again',
  exportResult: 'Export Result',
  exportName: 'Export {name}',
  everyRowTable: 'Every row of the table',
  rowCount: { one: '{count} row', other: '{count} rows' },
  rowsWritten: { one: '{count} row written', other: '{count} rows written' },
  rowsExported: { one: '{count} row exported', other: '{count} rows exported' },
});
