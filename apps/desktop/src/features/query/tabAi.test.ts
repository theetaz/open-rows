// SPDX-License-Identifier: Apache-2.0
import { act, renderHook } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { profile } from '../../test/ipcStub';
import { explainable, useAiActions } from './tabAi';

vi.mock('../assistant/conversation', () => ({ toggleAssistant: vi.fn() }));

it('opens no AI on a MongoDB connection and says why', async () => {
  const { toggleAssistant } = await import('../assistant/conversation');
  const mongo = profile('docs', { engine: 'mongodb' });
  const editor = { current: null };
  const { result } = renderHook(() => useAiActions(mongo, 'tab', editor));
  expect(result.current[0]).toBeNull();
  act(() => {
    result.current[1].onAssistant();
  });
  expect(result.current[0]).toBe('AI is not available for MongoDB connections');
  expect(toggleAssistant).not.toHaveBeenCalled();
  expect(explainable(mongo)).toBe(false);
});

it('opens the assistant on a SQL connection', async () => {
  const { toggleAssistant } = await import('../assistant/conversation');
  const editor = { current: null };
  const { result } = renderHook(() =>
    useAiActions(profile('orders'), 'tab', editor),
  );
  act(() => {
    result.current[1].onAssistant();
  });
  expect(toggleAssistant).toHaveBeenCalledWith('tab');
  expect(result.current[0]).toBeNull();
  expect(explainable(profile('orders'))).toBe(true);
});
