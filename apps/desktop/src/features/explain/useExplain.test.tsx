// SPDX-License-Identifier: Apache-2.0
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { cancelQuery, explainQuery } from '@openrows/ipc';
import type { ExplainReply, ExplainRequest } from '@openrows/ipc';
import { samplePlan } from '../../test/planStub';
import { explainRequest, useExplain } from './useExplain';
import type { ExplainSource } from './useExplain';
import { setExplain } from './explainStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const explain = vi.mocked(explainQuery);

afterEach(() => {
  setExplain('tab', null);
  explain.mockReset();
});

function source(text: string, selection = '', cursor = 0): ExplainSource {
  return { text: () => text, selection: () => selection, cursor: () => cursor };
}

function renderExplain(text = 'select 1; delete from t') {
  const editor = { current: source(text, '', 12) };
  return renderHook(() => useExplain('tab', 'session-1', editor));
}

const sent = (call: number): ExplainRequest =>
  explain.mock.calls[call]?.[0] ?? ({} as ExplainRequest);

it('explains the selection, else the statement at the cursor', () => {
  const selected = explainRequest(
    source('select 1; select 2', 'select 2'),
    's',
    false,
  );
  expect(selected).toMatchObject({
    sql: 'select 2',
    cursor: null,
    analyze: false,
  });
  const atCursor = explainRequest(
    source('select 1; select 2', '', 14),
    's',
    true,
  );
  expect(atCursor).toMatchObject({
    sql: 'select 1; select 2',
    cursor: 14,
    analyze: true,
  });
  expect(explainRequest(source('  '), 's', false)).toBeNull();
});

it('keeps the plan of a finished explain', async () => {
  const plan = samplePlan();
  explain.mockResolvedValue({ kind: 'explained', plan });
  const hook = renderExplain();
  act(() => {
    hook.result.current.run(false);
  });
  expect(hook.result.current.state).toMatchObject({
    status: 'running',
    analyze: false,
  });
  await waitFor(() => {
    expect(hook.result.current.state).toMatchObject({ status: 'done', plan });
  });
  act(() => {
    hook.result.current.close();
  });
  expect(hook.result.current.state).toBeNull();
});

it('asks before analysing a write and resends with the token', async () => {
  const reply: ExplainReply = {
    kind: 'confirmationRequired',
    token: 'tok',
    statement: 'delete from t',
  };
  explain.mockResolvedValueOnce(reply).mockReturnValue(new Promise(() => {}));
  const hook = renderExplain();
  act(() => {
    hook.result.current.run(true);
  });
  await waitFor(() => {
    expect(hook.result.current.state?.status).toBe('confirm');
  });
  act(() => {
    hook.result.current.confirm();
  });
  expect(sent(1)).toMatchObject({ analyze: true, confirmToken: 'tok' });
  expect(sent(1).explainId).not.toBe(sent(0).explainId);
  act(() => {
    hook.result.current.cancel();
  });
  expect(cancelQuery).toHaveBeenCalledWith(sent(1).explainId);
});

it('shows refusals and failures, and ignores replies it no longer waits for', async () => {
  explain.mockResolvedValueOnce({ kind: 'blocked', message: 'read-only' });
  const hook = renderExplain();
  act(() => {
    hook.result.current.run(true);
  });
  await waitFor(() => {
    expect(hook.result.current.state).toMatchObject({
      status: 'failed',
      message: 'read-only',
    });
  });
  let reject: (error: Error) => void = () => {};
  explain.mockReturnValueOnce(new Promise((_, fail) => (reject = fail)));
  act(() => {
    hook.result.current.run(false);
    hook.result.current.close();
  });
  await act(async () => {
    reject(new Error('late'));
    await Promise.resolve();
  });
  expect(hook.result.current.state).toBeNull();
});
