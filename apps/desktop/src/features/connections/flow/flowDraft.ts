// SPDX-License-Identifier: Apache-2.0
import type { EngineDto, EnvKindDto } from '@openrows/ipc';
import { emptyDraft } from '../form';
import type { Draft, DraftErrors } from '../form';
import { isFileEngine } from '../engines';
import { t } from './messages';

/** The five steps of the flow; file engines skip `security`. */
export type StepId = 'engine' | 'details' | 'security' | 'options' | 'test';

const SERVER_STEPS: readonly StepId[] = [
  'engine',
  'details',
  'security',
  'options',
  'test',
];
const FILE_STEPS: readonly StepId[] = ['engine', 'details', 'options', 'test'];

/** The steps of a flow; a saved connection keeps its engine, so an edit skips that step. */
export function stepOrder(
  engine: EngineDto,
  editing: boolean,
): readonly StepId[] {
  const order = isFileEngine(engine) ? FILE_STEPS : SERVER_STEPS;
  return editing ? order.slice(1) : order;
}

/** Which step shows each validated field, so Continue and Save land on it. */
const ERROR_STEP: Record<keyof DraftErrors, StepId> = {
  name: 'details',
  host: 'details',
  port: 'details',
  user: 'details',
  filePath: 'details',
  commandProgram: 'details',
  commandArgs: 'details',
  commandTimeout: 'details',
  cloudRegion: 'details',
  helper: 'security',
  sshHost: 'security',
  sshPort: 'security',
  sshKeyFile: 'security',
  sshJumpHosts: 'security',
  tlsCaFile: 'security',
  tlsClientKey: 'security',
  proxyHost: 'security',
  proxyPort: 'security',
  envLabel: 'options',
};

/** The steps holding an error, in flow order. */
export function stepsWithErrors(
  errors: DraftErrors,
  order: readonly StepId[],
): StepId[] {
  const keys = Object.keys(errors) as (keyof DraftErrors)[];
  const steps = new Set(keys.map((key) => ERROR_STEP[key]));
  return order.filter((step) => steps.has(step));
}

/** A new connection switched to another engine: its defaults, keeping identity and environment. */
export function withEngine(draft: Draft, engine: EngineDto): Draft {
  if (draft.engine === engine) return draft;
  return {
    ...emptyDraft(engine),
    name: draft.name,
    group: draft.group,
    tags: draft.tags,
    envKind: draft.envKind,
    envLabel: draft.envLabel,
    envColor: draft.envColor,
    safety: draft.safety,
    recordHistory: draft.recordHistory,
    redactHistory: draft.redactHistory,
  };
}

/** The name a connection gets when none is typed: the file, or host · database. */
export function derivedName(draft: Draft): string {
  if (isFileEngine(draft.engine)) {
    if (draft.fileMemory) return t('inMemory');
    return draft.filePath.split(/[\\/]/).at(-1) || t('untitled');
  }
  const host = draft.host.trim() || t('untitled');
  const database = draft.database.trim();
  return database ? `${host} · ${database}` : host;
}

/** A pasted URL's environment, guessed from its host name. */
export function guessEnvironment(host: string): EnvKindDto {
  if (/prod/i.test(host)) return 'prod';
  if (/stag|stg/i.test(host)) return 'staging';
  if (!host || /^(localhost|127\.|::1|\/)/.test(host)) return 'local';
  return 'dev';
}
