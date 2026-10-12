// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import { readSqlFile } from '@openrows/ipc';
import { copyProjectSql } from './sqlClipboard';
type OpenedSqlFileDto = Awaited<ReturnType<typeof readSqlFile>>;
vi.mock('@openrows/ipc', () => ({ readSqlFile: vi.fn() }));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it('starts the clipboard write during the click while SQL is still loading', async () => {
  let complete: (file: OpenedSqlFileDto) => void = () => undefined;
  const loading = new Promise<OpenedSqlFileDto>((resolve) => {
    complete = resolve;
  });
  vi.mocked(readSqlFile).mockReturnValueOnce(loading);
  let data: Record<string, Promise<Blob>> = {};
  vi.stubGlobal(
    'ClipboardItem',
    vi.fn(function Item(body: Record<string, Promise<Blob>>) {
      data = body;
      return {};
    }),
  );
  const write = vi.fn(() => Promise.resolve());
  vi.stubGlobal('navigator', { clipboard: { write } });
  const copied = copyProjectSql('/p/queries/top.sql');
  expect(write).toHaveBeenCalledOnce();
  const sql = "SELECT '${1:literal}', :id;";
  complete({
    sql,
    file: { name: 'top.sql', path: '/p/queries/top.sql', modifiedMs: null },
  });
  const blob = await data['text/plain'];
  expect(blob?.type).toBe('text/plain');
  expect(blob?.size).toBe(sql.length);
  if (!blob) throw new Error('Missing SQL clipboard body');
  const text = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result === 'string') resolve(reader.result);
      else reject(new Error('Clipboard fixture is not text'));
    };
    reader.onerror = () => {
      reject(new Error('Cannot read clipboard fixture'));
    };
    reader.readAsText(blob);
  });
  expect(text).toBe(sql);
  await copied;
});
