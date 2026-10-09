// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { profile, server } from '../../test/ipcStub';
import { insecureDetail, insecureReasons } from './insecure';
import { expiryText, latencyQuality } from './testReportText';

it('joins saved and live reasons once each', () => {
  const unverified = profile('db', { insecure: ['tlsUnverified'] });
  expect(insecureReasons(unverified, undefined)).toEqual(['tlsUnverified']);
  const live = {
    state: 'connected' as const,
    server: server('17.2'),
    insecure: ['tlsUnverified', 'hostKeyNotSaved'] as const,
    databases: [],
    generation: 1,
  };
  expect(insecureReasons(unverified, live)).toEqual([
    'tlsUnverified',
    'hostKeyNotSaved',
  ]);
  expect(insecureReasons(profile('safe'), { state: 'connecting' })).toEqual([]);
  expect(insecureDetail(['tlsDisabled', 'hostKeyNotSaved'])).toBe(
    'TLS is off, so traffic to the server is readable. The SSH host key was accepted once without saving it.',
  );
});

it('rates latency and words certificate expiry', () => {
  expect([12, 50, 199, 200].map(latencyQuality)).toEqual([
    'fast',
    'moderate',
    'moderate',
    'slow',
  ]);
  expect(expiryText(null)).toBeNull();
  expect(expiryText(0)).toBe('expires today');
  expect(expiryText(1)).toBe('expires in 1 day');
  expect(expiryText(45)).toBe('expires in 45 days');
  expect(expiryText(-2)).toBe('expired 2 days ago');
});
