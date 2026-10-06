// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { workspaceMonogram } from './monogram';
it.each([
  ['orders-staging', 'OS'],
  ['analytics-prod', 'AP'],
  ['app.sqlite', 'AS'],
  ['events', 'EV'],
  ['a', 'AA'],
  ['  weekly  reports ', 'WR'],
  ['東京-大阪', '東大'],
  ['école', 'ÉC'],
  ['සිංහල', 'සිංහ'],
  ['north-west-prod', 'NW'],
  ['e\u0301cole', 'ÉC'],
  ['𐐨𐐩', '𐐀𐐁'],
  ['', 'WS'],
  ['  ...--- ', 'WS'],
])('derives %s as %s without changing its input', (input, expected) => {
  expect(workspaceMonogram(input)).toBe(expected);
  expect(workspaceMonogram(input)).toBe(expected);
});
