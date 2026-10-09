// SPDX-License-Identifier: Apache-2.0
import type { ParamValueDto } from '@openrows/ipc';

/** How a parameter value is entered: as text, as a number, or SQL NULL. */
export type ParamKind = 'text' | 'number' | 'null';

/** A value the user entered for one parameter; `value` is kept while NULL. */
export type ParamValue = { kind: ParamKind; value: string };

/** A tab's parameter values by name (`:id`, `$1`, `?1`). */
export type ParamValues = Readonly<Record<string, ParamValue>>;

const KINDS: readonly ParamKind[] = ['text', 'number', 'null'];

const NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

/** Whether a value can be sent: a number must look like one. */
export function isValidParam(param: ParamValue): boolean {
  return param.kind !== 'number' || NUMBER.test(param.value.trim());
}

function paramOf(stored: unknown): ParamValue | null {
  if (typeof stored === 'string') return { kind: 'text', value: stored };
  if (typeof stored !== 'object' || stored === null) return null;
  const { kind, value } = stored as Record<string, unknown>;
  if (!KINDS.includes(kind as ParamKind) || typeof value !== 'string')
    return null;
  return { kind: kind as ParamKind, value };
}

/**
 * Stored parameter values, dropping invalid entries; plain strings (saved
 * before typed values existed) are text values.
 */
export function parseParamValues(stored: unknown): ParamValues | undefined {
  if (typeof stored !== 'object' || stored === null || Array.isArray(stored))
    return undefined;
  const entries = Object.entries(stored).flatMap(([name, item]) => {
    const param = paramOf(item);
    return param ? [[name, param] as const] : [];
  });
  return Object.fromEntries(entries);
}

function sentValue(param: ParamValue): string | null {
  if (param.kind === 'null') return null;
  return param.kind === 'number' ? param.value.trim() : param.value;
}

/**
 * The values the core binds. With `shift` (anonymous `?` only), the
 * document's `?n` become the run's `?(n - shift)`, as the run starts after
 * `shift` of them and the core numbers them from its first statement.
 */
export function paramDtos(values: ParamValues, shift = 0): ParamValueDto[] {
  return Object.entries(values).flatMap(([name, param]) => {
    const sent = sentValue(param);
    const index = /^\?(\d+)$/.exec(name)?.[1];
    if (index === undefined || shift === 0) return [{ name, value: sent }];
    const moved = Number(index) - shift;
    return moved > 0 ? [{ name: `?${String(moved)}`, value: sent }] : [];
  });
}
