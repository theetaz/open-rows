// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { mergeSessionTabs } from './useSavedTabs';
import { emptyTabs, openTab } from './tabs';
it('manual reopening preserves current edits, remaps colliding ids and restores split targets', () => {
  const scope = { connectionId: 'a', database: 'orders' };
  let saved = openTab(emptyTabs, scope, { sql: 'saved' });
  saved = openTab(saved, scope, { sql: 'second' });
  saved = {
    ...saved,
    splits: {
      'a\norders': {
        orientation: 'right',
        pinned: 'query-1',
        ratio: 0.5,
        focus: 'active',
      },
    },
  };
  const current = openTab(emptyTabs, scope, { sql: 'edited' });
  const reopened = mergeSessionTabs(current, saved);
  expect(reopened.tabs.map((tab) => tab.sql)).toEqual([
    'edited',
    'saved',
    'second',
  ]);
  expect(new Set(reopened.tabs.map((tab) => tab.id)).size).toBe(3);
  expect(reopened.splits?.['a\norders']?.pinned).toBe('query-2');
  expect(mergeSessionTabs(reopened, saved).tabs).toHaveLength(3);
});
