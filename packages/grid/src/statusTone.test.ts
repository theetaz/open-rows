// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { statusTone } from './statusTone';

it('reads state-like enum labels in any case and spelling', () => {
  expect(statusTone('SUCCESS')).toBe('ok');
  expect(statusTone('failed')).toBe('bad');
  expect(statusTone('In-Progress')).toBe('run');
  expect(statusTone('QUEUED')).toBe('wait');
  expect(statusTone('QUEUED')).toBe('wait');
});

it('leaves other labels plain', () => {
  expect(statusTone('weather_digest')).toBeNull();
  expect(statusTone('a label far too long to be a state')).toBeNull();
});
