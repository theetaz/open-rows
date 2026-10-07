// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { ChangeClock, measureKeystroke } from './editorSampler';
import type { PaintTiming, Probe, Target } from './editorSampler';

interface Fake {
  probe: Probe;
  input: { type: ReturnType<typeof vi.fn>; press: ReturnType<typeof vi.fn> };
}

/** Clock reads: dispatch 100; the input "updates" at `updateAt`; paint as given. */
function fake(paint: PaintTiming, updateAt: number | null, delta = 1): Fake {
  let length = 10;
  const changes = new ChangeClock(() => updateAt ?? 0);
  const edit = (): boolean => {
    length += delta;
    if (updateAt !== null) changes.record();
    return true;
  };
  const input = { type: vi.fn(edit), press: vi.fn(edit) };
  const probe: Probe = {
    input,
    frames: { now: () => 100, nextPaint: () => Promise.resolve(paint) },
    changes,
    docLength: () => length,
  };
  return { probe, input };
}

const insert: Target = {
  index: 7,
  position: 'middle',
  stroke: { kind: 'insert', key: 'a' },
  scheduledAt: 98,
};

it('splits a keystroke into queue, input, idle wait and frame', async () => {
  const { probe, input } = fake(
    { frameAt: 110, callbackAt: 111, paintedAt: 114 },
    103,
  );
  const sample = await measureKeystroke(probe, insert);
  expect(input.type).toHaveBeenCalledWith('a');
  expect(sample).toEqual({
    index: 7,
    position: 'middle',
    kind: 'insert',
    key: 'a',
    queueMs: 2,
    inputMs: 3,
    vsyncWaitMs: 7,
    frameMs: 4,
    wallMs: 14,
    keystrokeToPaintMs: 9,
  });
});

it('counts no idle wait when the update lands inside the frame', async () => {
  const { probe } = fake(
    { frameAt: 101, callbackAt: 102, paintedAt: 110 },
    104,
  );
  const sample = await measureKeystroke(probe, { ...insert, scheduledAt: 120 });
  expect(sample.vsyncWaitMs).toBe(0);
  expect(sample.queueMs).toBe(0);
  expect(sample.keystrokeToPaintMs).toBe(sample.wallMs);
});

it('clamps a frame timestamp later than its callback', async () => {
  const { probe } = fake(
    { frameAt: 130, callbackAt: 112, paintedAt: 115 },
    101,
  );
  const sample = await measureKeystroke(probe, insert);
  expect(sample.vsyncWaitMs).toBe(11);
  expect(sample.frameMs).toBe(3);
});

it('presses delete keys and expects one character fewer', async () => {
  const { probe, input } = fake(
    { frameAt: 105, callbackAt: 105, paintedAt: 108 },
    101,
    -1,
  );
  const target: Target = { ...insert, stroke: { kind: 'delete', key: 'x' } };
  const sample = await measureKeystroke(probe, target);
  expect(input.press).toHaveBeenCalledWith('x');
  expect(sample.kind).toBe('delete');
});

it('fails closed on refused, missing or wrong edits', async () => {
  const paint = { frameAt: 105, callbackAt: 105, paintedAt: 108 };
  const refused = fake(paint, 101);
  refused.input.type.mockReturnValue(false);
  await expect(measureKeystroke(refused.probe, insert)).rejects.toThrow(
    'refused insertText',
  );
  await expect(
    measureKeystroke(fake(paint, null).probe, insert),
  ).rejects.toThrow('missed its frame');
  await expect(
    measureKeystroke(fake(paint, 101, 2).probe, insert),
  ).rejects.toThrow('wrong text');
});

it('keeps only the first change after arming', () => {
  let now = 5;
  const clock = new ChangeClock(() => now);
  clock.record();
  now = 9;
  clock.record();
  expect(clock.first).toBe(5);
  clock.arm();
  expect(clock.first).toBeNull();
  clock.record();
  expect(clock.first).toBe(9);
});
