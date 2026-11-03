// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { richn, t } = defineMessages({
  tablesStagedEdits: 'Tables with staged edits',
  strongNamesCount: {
    one: '{strong} on {names}, {count} staged change',
    other: '{strong} on {names}, {count} staged changes',
  },
  strongNamesCountStaged: {
    one: '{strong} on {names}, {count} staged change, saved {value}',
    other: '{strong} on {names}, {count} staged changes, saved {value}',
  },
  readingJournal: 'Reading the journal…',
  journalCannotRead: 'The journal cannot be read: {message}',
  noStagedEdits: 'No staged edits were left behind.',
  openrowsClosedUnexpectedly: 'OpenRows closed unexpectedly',
  lastRunDid:
    'The last run did not exit cleanly. Staged table edits were kept in the local journal; open each table to review and restore them.',
  openLogsFolder: 'Open logs folder',
  done: 'Done',
  unknownConnection: 'an unknown connection',
});
