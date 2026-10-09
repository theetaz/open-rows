// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile } from '../../test/ipcStub';
import {
  emptyLimits,
  idleOptions,
  limitsFromProfile,
  limitsInput,
  limitsSummary,
  secondsLabel,
  statementOptions,
} from './limitsForm';

it('round-trips the profile limits; blank follows the default', () => {
  const own = profile('orders', {
    limits: { statementTimeoutSeconds: 45, idleTimeoutMinutes: 15 },
  });
  const draft = limitsFromProfile(own);
  expect(draft).toEqual({ statementTimeout: '45', idleTimeout: '15' });
  expect(limitsInput(draft)).toEqual(own.limits);
  expect(limitsInput(emptyLimits())).toEqual({
    statementTimeoutSeconds: null,
    idleTimeoutMinutes: 0,
  });
  expect(limitsInput({ statementTimeout: '0', idleTimeout: 'x' })).toEqual({
    statementTimeoutSeconds: 0,
    idleTimeoutMinutes: 0,
  });
});

it('labels choices, keeps an unlisted saved value and summarises', () => {
  expect(secondsLabel(0)).toBe('Off');
  expect(secondsLabel(45)).toBe('45 s');
  expect(secondsLabel(300)).toBe('5 min');
  expect(secondsLabel(7200)).toBe('2 h');
  const statement = statementOptions('45', 30);
  expect(statement[0]).toEqual({ value: '', label: 'Default (30 s)' });
  expect(statement.at(-1)).toEqual({ value: '45', label: '45 s' });
  expect(idleOptions('0')[0]).toEqual({ value: '0', label: 'Never' });
  expect(idleOptions('15').some((option) => option.label === '15 min')).toBe(
    true,
  );
  expect(limitsSummary(emptyLimits())).toBe('Defaults');
  expect(limitsSummary({ statementTimeout: '30', idleTimeout: '60' })).toBe(
    'Timeout 30 s · Idle 1 h',
  );
});
