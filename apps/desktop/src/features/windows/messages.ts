// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t, tn } = defineMessages({
  windowNumberTitle: 'Window {number} · {title}',
  windowNumber: 'Window {number}',
  tabWasNot: 'The tab was not moved',
  ok: 'OK',
  commitProgress: 'Commit in progress',
  moveTabChanges: 'Move tab with changes?',
  keepTabHere: 'Keep tab here',
  moveTab: 'Move tab',
  keepTabHereUntil:
    'Keep this tab here until the commit finishes; then move it.',
  monitor: 'Monitor',
  processes: 'Processes',
  journaledEdits: {
    one: '{count} staged row edit stays in the local journal; the table offers to restore it in the other window.',
    other:
      '{count} staged row edits stay in the local journal; the table offers to restore them in the other window.',
  },
  lostEdits: {
    one: '{count} staged result edit or structure change cannot move and will be discarded.',
    other:
      '{count} staged result edits or structure changes cannot move and will be discarded.',
  },
  databaseNotChanged: 'The database is not changed.',
});
