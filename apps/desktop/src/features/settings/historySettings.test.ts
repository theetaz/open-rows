// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import schema from '../../../src-tauri/settings.schema.json';
import { applySettings, collectSettings } from './settingsFile';
import { parseHistorySettings } from './historySettings';
it('validates retention, mirrors settings and declares its schema', () => {
  expect(
    parseHistorySettings({
      maxEntries: -1,
      maxAgeDays: 0.5,
      keepForever: 'true',
    }),
  ).toEqual({ maxEntries: 100000, maxAgeDays: 90, keepForever: false });
  applySettings({
    openrows: 1,
    history: { maxEntries: 25, maxAgeDays: 7, keepForever: true },
  });
  expect(collectSettings().history).toEqual({
    maxEntries: 25,
    maxAgeDays: 7,
    keepForever: true,
  });
  expect(schema.properties.history.properties.maxEntries.minimum).toBe(1);
  expect(schema.properties.history.properties.keepForever.default).toBe(false);
});
