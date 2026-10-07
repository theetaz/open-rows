// SPDX-License-Identifier: Apache-2.0
import { vi } from 'vitest';
import type { ProfileDto, ServerDto } from '@openrows/ipc';
import { catalogStub } from './catalogStub';

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
    database: 'orders',
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

/** What a synthetic PostgreSQL server reports after connecting. */
export function server(serverVersion: string, database = 'orders'): ServerDto {
  return { product: 'PostgreSQL', serverVersion, flavour: null, database };
}

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
    openConnection: vi.fn((profileId: string) =>
      Promise.resolve({
        server: server('17.2', 'postgres'),
        database:
          profiles.find((item) => item.id === profileId)?.database ?? null,
      }),
    ),
    closeConnection: vi.fn(() => Promise.resolve()),
    openSession: vi.fn((profileId: string, database: string) =>
      Promise.resolve({
        sessionId: `session-${profileId}-${database}`,
        server: server('17.2', database),
      }),
    ),
    ...catalogStub(),
    closeSession: vi.fn(() => Promise.resolve()),
    parsePostgresUrl: vi.fn(),
    testConnection: vi.fn(),
    createProfile: vi.fn(),
    updateProfile: vi.fn(),
    duplicateProfile: vi.fn(),
    deleteProfile: vi.fn(() => Promise.resolve()),
    confirmDestructive: vi.fn(() => Promise.resolve(null)),
    CommandError: class CommandError extends Error {},
    executeQuery: vi.fn(),
    cancelQuery: vi.fn(() => Promise.resolve()),
    releaseResults: vi.fn(() => Promise.resolve()),
    recentHistory: vi.fn(() => Promise.resolve([])),
  };
}
