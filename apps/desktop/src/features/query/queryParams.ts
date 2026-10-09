// SPDX-License-Identifier: Apache-2.0
import type { EditorDialect } from '@openrows/editor';
import type { ParamsDto, ParamStyleDto, ParamValueDto } from '@openrows/ipc';
import type { QueryParams } from './editorSettings';

/**
 * Placeholder styles offered per engine: `$1` is PostgreSQL's own syntax,
 * `?` MySQL's; a bare `?` is a `jsonb` operator in PostgreSQL, so it is not
 * offered there. SQLite understands all of them.
 */
const OFFERED: Record<EditorDialect, readonly ParamStyleDto[]> = {
  postgres: ['named', 'numbered'],
  mysql: ['named', 'anonymous'],
  mariadb: ['named', 'anonymous'],
  sqlite: ['named', 'numbered', 'anonymous'],
};

const LABELS: Record<EditorDialect, Record<ParamStyleDto, string>> = {
  postgres: { named: ':name', numbered: '$1', anonymous: '?' },
  mysql: { named: ':name', numbered: '$1', anonymous: '?' },
  mariadb: { named: ':name', numbered: '$1', anonymous: '?' },
  sqlite: { named: ':name', numbered: '?1', anonymous: '?' },
};

/** Menu choices of Settings ▸ Query Params for an engine. */
export function paramChoices(
  dialect: EditorDialect,
): (readonly [string, QueryParams])[] {
  return [
    ['Off', 'off'],
    ...OFFERED[dialect].map(
      (style) => [LABELS[dialect][style], style] as const,
    ),
  ];
}

/** The setting as it applies to an engine; a style it does not offer is off. */
export function effectiveParams(
  setting: QueryParams,
  dialect: EditorDialect,
): ParamStyleDto | null {
  return setting !== 'off' && OFFERED[dialect].includes(setting)
    ? setting
    : null;
}

/** `params` of a run: the style's values, or `null` to run SQL as written. */
export function runParams(
  style: ParamStyleDto | null,
  values: ParamValueDto[],
): ParamsDto | null {
  return style ? { style, values } : null;
}
