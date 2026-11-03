// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  selectRows: 'Select rows',
  selectJoin: 'Select with join',
  countRows: 'Count rows',
  insertRow: 'Insert a row',
  updateRows: 'Update rows',
  deleteRows: 'Delete rows',
  createTableIdentity: 'Create table (identity key)',
  upsertConflict: 'Upsert (ON CONFLICT)',
  explainAnalyze: 'Explain analyze',
  activeSessions: 'Active sessions',
  createTableAuto: 'Create table (auto increment key)',
  upsertDuplicateKey: 'Upsert (ON DUPLICATE KEY)',
  runningThreads: 'Running threads',
  firstRows: 'First rows',
  runningRequests: 'Running requests',
  createTableRowid: 'Create table (rowid key)',
  explainQueryPlan: 'Explain query plan',
  listTables: 'List tables',
  nameMust1: 'Name must be 1–{maxName} characters.',
  prefixMust1: 'Prefix must be 1–32 letters, digits, _ or $.',
  bodyMustNot: 'Body must not be empty.',
  noValidSnippets: 'No valid snippets found in the JSON.',
});
