// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { ProcessDto } from '@openrows/ipc';
import {
  DEFAULT_SORT,
  PROCESS_COLUMNS,
  filterProcesses,
  formatDuration,
  nextSort,
  processFields,
  processLabel,
  sortProcesses,
} from './processModel';
import { signalPrompt } from './processSignals';
import { sampleProcess as process } from '../../test/processStub';

const col = (key: keyof ProcessDto): number =>
  PROCESS_COLUMNS.findIndex((column) => column.key === key);

it('formats durations from milliseconds to hours', () => {
  expect(formatDuration(null)).toBe('');
  expect(formatDuration(850)).toBe('850 ms');
  expect(formatDuration(12_400)).toBe('12.4 s');
  expect(formatDuration(185_000)).toBe('3 m 05 s');
  expect(formatDuration(7_620_000)).toBe('2 h 07 m');
});

it('filters on any column, case-insensitively', () => {
  const list = [
    process(1, { query: 'SELECT pg_sleep(60)' }),
    process(2, { user: 'Reporting' }),
  ];
  expect(filterProcesses(list, ' PG_SLEEP ').map((item) => item.id)).toEqual([
    1,
  ]);
  expect(filterProcesses(list, 'reporting').map((item) => item.id)).toEqual([
    2,
  ]);
  expect(filterProcesses(list, '')).toHaveLength(2);
});

it('sorts numbers and text, keeps empty values last and ties by pid', () => {
  const list = [
    process(3, { durationMs: 500, user: 'b' }),
    process(1, { durationMs: null, user: null }),
    process(2, { durationMs: 9000, user: 'a' }),
    process(4, { durationMs: 500, user: 'a' }),
  ];
  const ids = (sorted: ProcessDto[]): number[] => sorted.map((item) => item.id);
  expect(ids(sortProcesses(list, DEFAULT_SORT))).toEqual([2, 3, 4, 1]);
  const byUser = { col: col('user'), descending: false };
  expect(ids(sortProcesses(list, byUser))).toEqual([2, 4, 3, 1]);
  expect(ids(sortProcesses(list, { ...byUser, descending: true }))).toEqual([
    3, 2, 4, 1,
  ]);
});

it('flips the sorted column and starts durations descending', () => {
  const byPid = nextSort(DEFAULT_SORT, col('id'));
  expect(byPid).toEqual({ col: col('id'), descending: false });
  expect(nextSort(byPid, col('id')).descending).toBe(true);
  expect(nextSort(byPid, col('durationMs')).descending).toBe(true);
});

it('gives the inspector every field with the full query text', () => {
  const query = 'SELECT *\n  FROM orders\n WHERE id = 1';
  const fields = processFields(process(7, { query, wait: null }));
  expect(fields.find((field) => field.name === 'Query')?.text).toBe(query);
  expect(fields.find((field) => field.name === 'Waiting for')?.state).toBe(
    'null',
  );
  expect(fields.find((field) => field.name === 'Duration')?.text).toBe('1.0 s');
});

it('names the process and warns about the app’s own connections', () => {
  expect(processLabel(process(7))).toBe('process 7 (app, psql)');
  expect(processLabel(process(7, { user: null, application: null }))).toBe(
    'process 7',
  );
  const kill = signalPrompt({ process: process(7), signal: 'terminate' });
  expect(kill).toMatchObject({
    title: 'Kill process 7 (app, psql)?',
    action: 'Kill Process',
  });
  expect(kill.message).toContain('rolled back');
  const own = signalPrompt({
    process: process(8, { current: true, app: true }),
    signal: 'cancelQuery',
  });
  expect(own.action).toBe('Cancel Query');
  expect(own.message).toContain('this process list runs on');
  const app = signalPrompt({
    process: process(9, { app: true }),
    signal: 'terminate',
  });
  expect(app.message).toContain('OpenRows’ own connections');
});
