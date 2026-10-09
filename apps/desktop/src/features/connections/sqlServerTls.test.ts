// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import {
  encryptLabel,
  encryptOf,
  modeOf,
  trustsCertificate,
} from './sqlServerTls';

it('maps Encrypt and Trust server certificate onto TLS modes', () => {
  expect(encryptOf('disable')).toBe('off');
  expect(encryptOf('allow')).toBe('login');
  expect(encryptOf('prefer')).toBe('optional');
  expect(encryptOf('verify-ca')).toBe('mandatory');
  expect(modeOf('mandatory', true, 'verify-full')).toBe('require');
  expect(modeOf('mandatory', false, 'require')).toBe('verify-full');
  expect(modeOf('mandatory', false, 'verify-ca')).toBe('verify-ca');
  expect(modeOf('login', true, 'require')).toBe('allow');
  expect(modeOf('off', false, 'prefer')).toBe('disable');
  expect(trustsCertificate('require')).toBe(true);
  expect(trustsCertificate('verify-full')).toBe(false);
});

it('labels the section like SQL Server tools', () => {
  expect(encryptLabel('require')).toBe('Mandatory, trusted certificate');
  expect(encryptLabel('verify-full')).toBe('Mandatory');
  expect(encryptLabel('prefer')).toBe('Optional');
  expect(encryptLabel('disable')).toBe('Off');
});
