// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, expect, it, vi } from 'vitest';
import { resolveQuerySource } from '@openrows/ipc';
import type { SourceResolution } from '@openrows/ipc';
import { useResultMapping } from './useResultMapping';
import type { ResultMeta } from './run';

vi.mock('@openrows/ipc', () => ({ resolveQuerySource: vi.fn() }));
const result = (id = 'r1'): ResultMeta => ({
  id,
  statement: 0,
  rows: 1,
  done: true,
  truncated: false,
  columns: [{ name: 'id', sourceType: 'int4' }],
});
beforeEach(() => {
  vi.resetAllMocks();
});

it('resolves once per result/session/statement, including remounts, and refreshes each run', async () => {
  vi.mocked(resolveQuerySource).mockResolvedValue({
    kind: 'table',
    schema: 'tenant',
    table: 'users',
  });
  const meta = result();
  const hook = renderHook(
    ({ meta, session, sql }) =>
      useResultMapping(meta, sql, 'postgres', session),
    {
      initialProps: { meta, session: 's1', sql: 'SELECT * FROM users' },
    },
  );
  await waitFor(() => {
    expect(hook.result.current).toMatchObject({
      kind: 'table',
      schema: 'tenant',
    });
  });
  hook.rerender({ meta, session: 's1', sql: 'SELECT * FROM users' });
  expect(resolveQuerySource).toHaveBeenCalledTimes(1);
  hook.unmount();
  const remount = renderHook(() =>
    useResultMapping(meta, 'SELECT * FROM users', 'postgres', 's1'),
  );
  await waitFor(() => {
    expect(remount.result.current.kind).toBe('table');
  });
  expect(resolveQuerySource).toHaveBeenCalledTimes(1);
  remount.unmount();
  vi.mocked(resolveQuerySource).mockResolvedValue({
    kind: 'table',
    schema: 'changed_path',
    table: 'users',
  });
  const next = result('r2');
  const rerun = renderHook(() =>
    useResultMapping(next, 'SELECT * FROM users', 'postgres', 's1'),
  );
  await waitFor(() => {
    expect(rerun.result.current).toMatchObject({ schema: 'changed_path' });
  });
  expect(resolveQuerySource).toHaveBeenCalledTimes(2);
});

it.each(['ambiguous name', 'missing table', 'view', 'Temporary table'])(
  'keeps %s read-only with the resolver reason',
  async (reason) => {
    vi.mocked(resolveQuerySource).mockResolvedValue({
      kind: 'readonly',
      reason,
    });
    const meta = result();
    const hook = renderHook(() =>
      useResultMapping(meta, 'SELECT * FROM users', 'postgres', 's1'),
    );
    await waitFor(() => {
      expect(hook.result.current).toEqual({ kind: 'readonly', reason });
    });
  },
);

it('reports resolution errors and does not re-query on rerender', async () => {
  vi.mocked(resolveQuerySource).mockRejectedValue(
    new Error('permission denied'),
  );
  const meta = result();
  const hook = renderHook(() =>
    useResultMapping(meta, 'SELECT * FROM users', 'postgres', 's1'),
  );
  await waitFor(() => {
    expect(hook.result.current.kind).toBe('readonly');
    if (hook.result.current.kind === 'readonly')
      expect(hook.result.current.reason).toContain('permission denied');
  });
  hook.rerender();
  expect(resolveQuerySource).toHaveBeenCalledTimes(1);
});

it('ignores a stale reply when the session changes and starts the new session read-only', async () => {
  let finish: ((source: SourceResolution) => void) | undefined;
  const pending = new Promise<SourceResolution>((resolve) => {
    finish = resolve;
  });
  vi.mocked(resolveQuerySource).mockReturnValueOnce(pending);
  vi.mocked(resolveQuerySource).mockResolvedValue({
    kind: 'readonly',
    reason: 'missing in new database',
  });
  const meta = result();
  const hook = renderHook(
    ({ session }) =>
      useResultMapping(meta, 'SELECT * FROM users', 'postgres', session),
    { initialProps: { session: 's1' } },
  );
  hook.rerender({ session: 's2' });
  expect(hook.result.current.kind).toBe('unresolved');
  await waitFor(() => {
    expect(hook.result.current).toMatchObject({ kind: 'readonly' });
  });
  await act(async () => {
    finish?.({ kind: 'table', schema: 'old', table: 'users' });
    await pending;
  });
  expect(hook.result.current).toMatchObject({ kind: 'readonly' });
  expect(resolveQuerySource).toHaveBeenNthCalledWith(2, 's2', 'users');
});

it('does not resolve incomplete, disconnected, qualified or unsafe results', () => {
  const meta = result();
  renderHook(() =>
    useResultMapping(
      { ...meta, done: false },
      'SELECT * FROM users',
      'postgres',
      's1',
    ),
  );
  renderHook(() =>
    useResultMapping(meta, 'SELECT * FROM users', 'postgres', null),
  );
  renderHook(() =>
    useResultMapping(meta, 'SELECT * FROM public.users', 'postgres', 's1'),
  );
  renderHook(() =>
    useResultMapping(meta, 'SELECT DISTINCT * FROM users', 'postgres', 's1'),
  );
  expect(resolveQuerySource).not.toHaveBeenCalled();
});
