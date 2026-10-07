// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { ProfileDto } from '@openrows/ipc';

/** A saved profile with synthetic, non-secret values. */
export function profile(
  name: string,
  overrides: Partial<ProfileDto> = {},
): ProfileDto {
  return {
    id: `id-${name}`,
    name,
    group: null,
    env: { kind: 'local', label: null, color: '#6b7280' },
    host: 'localhost',
    port: 5432,
    database: name,
    user: 'app',
    tlsMode: 'prefer',
    hasPassword: false,
    safety: { readOnly: false, confirmWrites: false },
    ...overrides,
  };
}

/** Two local profiles and one prod profile for workspace tests. */
export const sampleProfiles: readonly ProfileDto[] = [
  profile('orders-local'),
  profile('analytics-prod', {
    env: { kind: 'prod', label: null, color: '#dc2626' },
    safety: { readOnly: false, confirmWrites: true },
  }),
  profile('events-dev', {
    env: { kind: 'dev', label: null, color: '#16a34a' },
  }),
];

/** `@openrows/ipc` replacement for jsdom tests: no native shell, no keychain. */
export function ipcStub(
  profiles: readonly ProfileDto[] = sampleProfiles,
): Record<string, unknown> {
  return {
    DEFAULT_CUSTOM_ENV_COLOR: '#7c3aed',
    getAppInfo: vi.fn(() => new Promise(() => {})),
    listProfiles: vi.fn(() =>
      Promise.resolve({ profiles: [...profiles], unreadable: [] }),
    ),
    onSessionState: vi.fn(() => Promise.resolve(() => {})),
    openSession: vi.fn((profileId: string) =>
      Promise.resolve({
        sessionId: `session-${profileId}`,
        serverVersion: '17.2',
      }),
    ),
    closeSession: vi.fn(() => Promise.resolve()),
    parsePostgresUrl: vi.fn(),
    testConnection: vi.fn(),
    createProfile: vi.fn(),
    updateProfile: vi.fn(),
    duplicateProfile: vi.fn(),
    deleteProfile: vi.fn(() => Promise.resolve()),
  };
}
