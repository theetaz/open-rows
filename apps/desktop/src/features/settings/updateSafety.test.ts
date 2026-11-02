// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { updateIsSafe } from './updateSafety';

it('blocks replacement until all query and edit tabs have passed close guards', () => {
  expect(updateIsSafe(1, [])).toBe(false);
  expect(updateIsSafe(2, ['disconnected', 'failed'])).toBe(false);
});

it('blocks live and opening sessions across every profile', () => {
  expect(updateIsSafe(0, ['disconnected', 'connected'])).toBe(false);
  expect(updateIsSafe(0, ['connecting', 'failed'])).toBe(false);
});

it('allows empty/disconnected workspaces and historical connection failures', () => {
  expect(updateIsSafe(0, [])).toBe(true);
  expect(updateIsSafe(0, ['disconnected', 'failed'])).toBe(true);
});
