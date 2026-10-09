// SPDX-License-Identifier: Apache-2.0
import type { DumpToolKindDto } from '@openrows/ipc';
import { localSetting } from './localSetting';

/**
 * Settings ▸ Tools: a path per client tool; empty finds the installed copy
 * that best matches the server. `settings.json` keeps them under `tools`,
 * where the core reads them when a backup or restore starts.
 */
type ToolSettings = Readonly<Record<DumpToolKindDto, string>>;

export const TOOLS_KEY = 'openrows.tools';

/** Longest path accepted (the settings schema's `maxLength`). */
const MAX_PATH = 1024;

export const TOOL_KINDS: readonly DumpToolKindDto[] = [
  'pgDump',
  'pgRestore',
  'psql',
  'mysqldump',
  'mysql',
  'aws',
  'sessionManagerPlugin',
  'kubectl',
  'cloudSqlProxy',
  'alloydbAuthProxy',
  'cloudflared',
];

const defaults: ToolSettings = {
  pgDump: '',
  pgRestore: '',
  psql: '',
  mysqldump: '',
  mysql: '',
  aws: '',
  sessionManagerPlugin: '',
  kubectl: '',
  cloudSqlProxy: '',
  alloydbAuthProxy: '',
  cloudflared: '',
};

/** Validated tool paths; anything else falls back to automatic. */
export function parseTools(stored: unknown): ToolSettings {
  if (typeof stored !== 'object' || stored === null) return defaults;
  const record = stored as Record<string, unknown>;
  const paths = TOOL_KINDS.map((kind): [DumpToolKindDto, string] => {
    const value = record[kind];
    const path = typeof value === 'string' ? value.trim() : '';
    return [kind, path.length <= MAX_PATH ? path : ''];
  });
  return Object.fromEntries(paths) as ToolSettings;
}

export const toolSettings = localSetting(TOOLS_KEY, parseTools);

/** Set (or clear with `''`) the path of one tool. */
export function setToolPath(kind: DumpToolKindDto, path: string): void {
  toolSettings.set(parseTools({ ...toolSettings.get(), [kind]: path }));
}
