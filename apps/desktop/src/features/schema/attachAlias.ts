// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto } from '@openrows/ipc';

/** Schema names `SQLite` keeps for the main and temporary databases. */
const RESERVED = ['main', 'temp'];

/** Schema names of the profile's attached databases. */
export function attachedAliases(profile: ProfileDto | undefined): string[] {
  return profile?.file?.attached?.map(({ alias }) => alias) ?? [];
}

/** The attached alias matching `schema` (case-insensitive), if any. */
export function attachedSchema(
  profile: ProfileDto | undefined,
  schema: string,
): string | undefined {
  const wanted = schema.toLowerCase();
  return attachedAliases(profile).find(
    (alias) => alias.toLowerCase() === wanted,
  );
}

/**
 * A schema name for the file at `path`: its stem with anything but letters,
 * digits and `_` replaced, never starting with a digit, never `main`/`temp`
 * and unique among `taken` (case-insensitive), e.g. `/data/2024 logs.db` →
 * `_2024_logs`.
 */
export function aliasFor(path: string, taken: readonly string[]): string {
  const base = path.split(/[\\/]/).pop() ?? '';
  const stem = base.replace(/\.[^.]*$/, '') || base;
  const cleaned = stem.replace(/[^A-Za-z0-9_]+/g, '_').replace(/^_+|_+$/g, '');
  const named = cleaned || 'attached';
  const safe = /^[0-9]/.test(named) ? `_${named}` : named;
  const used = new Set(
    [...RESERVED, ...taken].map((name) => name.toLowerCase()),
  );
  let alias = safe;
  for (let suffix = 2; used.has(alias.toLowerCase()); suffix += 1)
    alias = `${safe}_${String(suffix)}`;
  return alias;
}
