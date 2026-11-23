// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  checkingApplicationLock: 'Checking application lock…',
  unrestricted: 'Unrestricted',
  confirmAll: 'Confirm all',
  confirmWrites: 'Confirm writes',
  authenticateAll: 'Authenticate all',
  authenticateWrites: 'Authenticate writes',
  runOrdinaryStatements:
    'Run ordinary statements without prompts; dangerous statements still confirm.',
  confirmEveryStatement: 'Confirm every statement, including reads.',
  confirmOperationsMay:
    'Confirm operations that may change data; default for production.',
  useOsAuthentication: 'Use OS authentication for every statement.',
  useOsAuthenticationEvery:
    'Use OS authentication for every non-read statement.',
  rejectEveryOperation: 'Reject every operation that may change data.',
  readOnlyFile: 'Read-only file',
  databaseFileOpen:
    'The database file is open read-only, so the engine refuses writes.',
});
