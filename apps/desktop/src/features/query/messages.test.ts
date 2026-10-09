// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { filterLevel, resultMessages } from './messages';
import type { ResultMeta, RunMessage } from './run';

const message = (
  statement: number,
  level: RunMessage['level'],
): RunMessage => ({
  statement,
  level,
  text: `${level} ${String(statement)}`,
  code: null,
});
const result = (statement: number): ResultMeta => ({
  id: `r${String(statement)}`,
  statement,
  columns: [],
  rows: 0,
  done: true,
  truncated: false,
});

it('shows the shown result’s messages plus those of statements without one', () => {
  const messages = [
    message(0, 'info'),
    message(1, 'warning'),
    message(2, 'error'),
  ];
  const results = [result(0), result(2)];
  const texts = (current: ResultMeta, showNotices = true): string[] =>
    resultMessages({ messages, results, current, showNotices }).map(
      (item) => item.text,
    );
  expect(texts(result(0))).toEqual(['info 0', 'warning 1']);
  expect(texts(result(2))).toEqual(['warning 1', 'error 2']);
  expect(texts(result(2), false)).toEqual(['error 2']);
  expect(
    resultMessages({
      messages,
      results: [result(0)],
      current: result(0),
      showNotices: true,
    }),
  ).toHaveLength(3);
});

it('filters messages by level', () => {
  const messages = [
    message(0, 'info'),
    message(0, 'warning'),
    message(0, 'error'),
  ];
  expect(filterLevel(messages, 'all')).toHaveLength(3);
  expect(filterLevel(messages, 'warning').map((item) => item.level)).toEqual([
    'warning',
  ]);
});
