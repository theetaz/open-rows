// SPDX-License-Identifier: Apache-2.0
import type { LimitsDto, ProfileDto } from '@openrows/ipc';

/**
 * Session limits as the connection form edits them: the statement timeout
 * in seconds (`''` follows the app-wide default, `'0'` is no limit) and the
 * idle timeout in minutes (`'0'` never closes the connection).
 */
export type LimitsDraft = { statementTimeout: string; idleTimeout: string };

type Option = { value: string; label: string };

const STATEMENT_SECONDS = [15, 30, 60, 300, 900, 3600];
const IDLE_MINUTES = [5, 15, 30, 60, 120, 480];

export function emptyLimits(): LimitsDraft {
  return { statementTimeout: '', idleTimeout: '0' };
}

export function limitsFromProfile(profile: ProfileDto): LimitsDraft {
  const { statementTimeoutSeconds, idleTimeoutMinutes } = profile.limits;
  return {
    statementTimeout:
      statementTimeoutSeconds === null ? '' : String(statementTimeoutSeconds),
    idleTimeout: String(idleTimeoutMinutes),
  };
}

/** A whole number of at least 0 from a draft value; `fallback` otherwise. */
function count(text: string, fallback: number): number {
  const value = Number(text);
  return Number.isInteger(value) && value >= 0 ? value : fallback;
}

export function limitsInput(draft: LimitsDraft): LimitsDto {
  return {
    statementTimeoutSeconds:
      draft.statementTimeout === '' ? null : count(draft.statementTimeout, 0),
    idleTimeoutMinutes: count(draft.idleTimeout, 0),
  };
}

/** `30 s`, `5 min` or `1 h`; `Off` for `0`. */
export function secondsLabel(seconds: number): string {
  if (seconds === 0) return 'Off';
  if (seconds < 60 || seconds % 60 !== 0) return `${String(seconds)} s`;
  const minutes = seconds / 60;
  return minutes < 60 || minutes % 60 !== 0
    ? `${String(minutes)} min`
    : `${String(minutes / 60)} h`;
}

/** Choices plus the saved value when it is not one of them. */
function withCurrent(
  options: readonly Option[],
  current: string,
  label: (value: number) => string,
): Option[] {
  return options.some((option) => option.value === current)
    ? [...options]
    : [...options, { value: current, label: label(count(current, 0)) }];
}

/** Statement timeout choices; the first follows the app-wide default. */
export function statementOptions(
  current: string,
  defaultSeconds: number,
): Option[] {
  const options = [
    { value: '', label: `Default (${secondsLabel(defaultSeconds)})` },
    { value: '0', label: 'No limit' },
    ...STATEMENT_SECONDS.map((seconds) => ({
      value: String(seconds),
      label: secondsLabel(seconds),
    })),
  ];
  return withCurrent(options, current, secondsLabel);
}

/** Idle timeout choices in minutes. */
export function idleOptions(current: string): Option[] {
  const options = [
    { value: '0', label: 'Never' },
    ...IDLE_MINUTES.map((minutes) => ({
      value: String(minutes),
      label: secondsLabel(minutes * 60),
    })),
  ];
  return withCurrent(options, current, (minutes) => secondsLabel(minutes * 60));
}

/** Disclosure summary, e.g. `Timeout 30 s · Idle 15 min`. */
export function limitsSummary(draft: LimitsDraft): string {
  const parts = [];
  if (draft.statementTimeout !== '')
    parts.push(`Timeout ${secondsLabel(count(draft.statementTimeout, 0))}`);
  if (draft.idleTimeout !== '0')
    parts.push(`Idle ${secondsLabel(count(draft.idleTimeout, 0) * 60)}`);
  return parts.length ? parts.join(' · ') : 'Defaults';
}
