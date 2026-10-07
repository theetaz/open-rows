// SPDX-License-Identifier: Apache-2.0
import { beforeEach, expect, it, vi } from 'vitest';
import type { ConfigContext } from './editorRun';
import { runEditorSpike } from './editor';
import { SpikeSession } from './lifecycle';

vi.mock('./timer', () => ({
  measurePageTimer: () => ({ resolutionMs: 1, crossOriginIsolated: false }),
}));
const mocks = vi.hoisted(() => ({
  finish: vi.fn(),
  progress: vi.fn(),
  run: vi.fn(),
}));
vi.mock('@openrows/ipc', () => ({
  finishSpike: mocks.finish,
  spikeProgress: mocks.progress,
}));
vi.mock('./editorRun', () => ({ runConfig: mocks.run }));
beforeEach(() => {
  vi.resetAllMocks();
  mocks.finish.mockResolvedValue(undefined);
  mocks.progress.mockResolvedValue(undefined);
});

it('runs both configurations, Vim only for vim, and reports once', async () => {
  const hosts: HTMLElement[] = [];
  mocks.run.mockImplementation((context: ConfigContext) => {
    hosts.push(context.host);
    expect(context.host.isConnected).toBe(true);
    expect(context.doc.split('\n')).toHaveLength(5_000);
    return Promise.resolve({
      config: context.config,
      vim: context.vim !== null,
    });
  });
  await runEditorSpike(new SpikeSession());
  expect(
    mocks.run.mock.calls.map(([context]) => (context as ConfigContext).config),
  ).toEqual(['sql', 'vim']);
  expect(hosts[0]?.isConnected).toBe(false);
  expect(mocks.finish).toHaveBeenCalledOnce();
  expect(mocks.finish).toHaveBeenCalledWith(
    expect.objectContaining({
      error: null,
      editor: [
        { config: 'sql', vim: false },
        { config: 'vim', vim: true },
      ],
    }),
  );
  expect(mocks.progress).toHaveBeenCalledWith('editor-config', 'vim complete');
});

it('keeps completed runs and reports the failing configuration', async () => {
  mocks.run
    .mockResolvedValueOnce({ config: 'sql' })
    .mockRejectedValueOnce(new Error('Vim did not enter insert mode'));
  await runEditorSpike(new SpikeSession());
  expect(mocks.finish).toHaveBeenCalledWith(
    expect.objectContaining({
      error: 'Error: Vim did not enter insert mode',
      editor: [{ config: 'sql' }],
    }),
  );
  expect(document.body.childElementCount).toBe(0);
});
