// SPDX-License-Identifier: Apache-2.0
import type { EditorView } from '@openrows/editor';
import * as vim from '@openrows/editor/vim';
import { afterEach, expect, it, vi } from 'vitest';
import { runConfig } from './editorRun';
import type { SpikeSession } from './lifecycle';
import { generateSqlDocument } from './sqlDocument';

// jsdom has no editing pipeline or frames: type through a transaction and paint instantly.
vi.mock('./editorFrames', () => {
  let clock = 0;
  const now = (): number => ++clock;
  return {
    browserFrames: {
      now,
      nextPaint: () => {
        const frameAt = now();
        return Promise.resolve({
          frameAt,
          callbackAt: now(),
          paintedAt: now(),
        });
      },
    },
    delay: () => Promise.resolve(),
    refreshInterval: () => Promise.resolve(16.7),
    viewInput: (view: EditorView) => ({
      type: (text: string) => {
        view.dispatch(view.state.replaceSelection(text));
        return true;
      },
      press: (key: string) =>
        view.contentDOM.dispatchEvent(
          new KeyboardEvent('keydown', {
            key,
            bubbles: true,
            cancelable: true,
          }),
        ),
    }),
  };
});

afterEach(() => {
  document.body.replaceChildren();
});

function session(): SpikeSession {
  return {
    signal: new AbortController().signal,
    progress: vi.fn(),
  } as unknown as SpikeSession;
}

it.each([
  ['sql', null],
  ['vim', vim],
] as const)(
  'types and deletes at all three sites in %s mode',
  async (config, module) => {
    const host = document.createElement('div');
    document.body.append(host);
    const doc = generateSqlDocument();
    const run = await runConfig({
      host,
      session: session(),
      config,
      doc,
      vim: module,
    });
    expect(run).toMatchObject({
      config,
      lines: 5_000,
      documentChars: doc.length,
      refreshIntervalMs: 16.7,
    });
    expect(run.samples).toHaveLength(240);
    expect(run.samples.map((sample) => sample.index)).toEqual([
      ...Array(240).keys(),
    ]);
    for (const position of ['top', 'middle', 'end']) {
      const site = run.samples.filter((sample) => sample.position === position);
      expect(site.filter((sample) => sample.kind === 'insert')).toHaveLength(
        60,
      );
      expect(site.filter((sample) => sample.kind === 'delete')).toHaveLength(
        20,
      );
    }
    expect(run.samples.at(-1)?.key).toBe(config === 'vim' ? 'x' : 'Backspace');
    expect(host.childElementCount).toBe(0);
  },
);

it('fails closed if Vim keys arrive without Vim mode', async () => {
  const host = document.createElement('div');
  document.body.append(host);
  await expect(
    runConfig({
      host,
      session: session(),
      config: 'vim',
      doc: generateSqlDocument(),
      vim: null,
    }),
  ).rejects.toThrow('without Vim mode');
  expect(host.childElementCount).toBe(0);
});

it('fails closed if the editor window never becomes key', async () => {
  const focus = vi.spyOn(document, 'hasFocus').mockReturnValue(false);
  const host = document.createElement('div');
  document.body.append(host);
  await expect(
    runConfig({
      host,
      session: session(),
      config: 'sql',
      doc: generateSqlDocument(),
      vim: null,
    }),
  ).rejects.toThrow('never gained keyboard focus');
  expect(host.childElementCount).toBe(0);
  focus.mockRestore();
});
