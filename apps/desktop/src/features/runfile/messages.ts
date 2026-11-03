// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t, tn } = defineMessages({
  runName: 'Run {name}',
  sizeBytes: '{bytes} bytes',
  statementsOfTotal: {
    one: '{count} statement of {total}',
    other: '{count} statements of {total}',
  },
  statementCount: { one: '{count} statement', other: '{count} statements' },
  errorCount: { one: '{count} error', other: '{count} errors' },
  statementsRan: {
    one: '{count} statement ran',
    other: '{count} statements ran',
  },
  ranWithErrors: '{ran}, {errors}',
  summaryRolledBack: '{summary}; rolled back',
  cancelledAfter: 'Cancelled after {ran}',
  cancelledAfterRolledBack: 'Cancelled after {ran}; rolled back',
});
