// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { rich, t, tn } = defineMessages({
  runName: 'Run {name}',
  cancelRun: 'Cancel Run',
  runBackground: 'Run in Background',
  done: 'Done',
  runProgress: 'Run progress',
  statementsTotalStatements: '{statements} of {total} statements',
  bytesCount: '{bytes} of {count}',
  transactionWasRolled: 'The transaction was rolled back.',
  errors: 'Errors',
  countErrorsFirst: {
    one: '{count} error (first {count2} shown)',
    other: '{count} errors (first {count2} shown)',
  },
  countErrors: { one: '{count} error', other: '{count} errors' },
  lineLine: 'Line {line}',
  bytesConnectionDatabase:
    '{bytes} on {connection} : {database} · {statements} statements',
  bytesConnectionDatabase2: '{bytes} on {connection} : {database}',
  cancel: 'Cancel',
  runAnyway: 'Run Anyway',
  run: 'Run',
  checkingEveryStatement: 'Checking every statement…',
  fileHoldsNo: 'The file holds no SQL statements.',
  connectionReadOnly:
    'This connection is read-only: {blocked} statements may modify the database, so nothing will run.',
  confirmStatementsMay:
    '{confirm} statements may modify data or the schema. Review them before running.',
  error: 'On error',
  stopFirstError: 'Stop at the first error',
  continueListEvery: 'Continue and list every error',
  runEverythingOne:
    'Run everything in one transaction (rolled back on error or cancel)',
  databaseCommitsSchema:
    'This database commits schema changes immediately, so a file cannot run in one transaction.',
  doneWithRows:
    'Done: {statements} of {total} statements ran, {rows} rows affected in {elapsed}.',
  doneRan: 'Done: {statements} of {total} statements ran in {elapsed}.',
  cancelledRan:
    'Cancelled: {statements} of {total} statements ran in {elapsed}.',
  stoppedWithErrors:
    'Stopped with errors: {statements} of {total} statements ran in {elapsed}.',
  reasonSentence: '{reason}.',
});
