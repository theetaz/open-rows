// SPDX-License-Identifier: Apache-2.0
import type { AlertRule, Incident } from '@openrows/ipc';

/** Ratios are fractions in core; show them as percentages. */
export function formatValue(metric: string, value: number): string {
  if (metric.endsWith('_ratio')) return `${(value * 100).toFixed(1)} %`;
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

/** Firing and recovery thresholds, e.g. "fires ≥ 5 · clears ≤ 1". */
export function thresholds(rule: AlertRule): string {
  const [fire, clear] = rule.comparison === 'above' ? ['≥', '≤'] : ['≤', '≥'];
  return `fires ${fire} ${formatValue(rule.metric, rule.fire)} · clears ${clear} ${formatValue(rule.metric, rule.clear)}`;
}

/** Required hold as samples and seconds. */
export function hold(value: { samples: number; ms: number }): string {
  const seconds = value.ms / 1000;
  return seconds
    ? `${String(value.samples)} samples, ${String(seconds)} s`
    : `${String(value.samples)} sample${value.samples === 1 ? '' : 's'}`;
}

/** Onset, peak and last values with the thresholds that applied. */
export function evidence(incident: Incident): string {
  const metric = incident.rule.metric;
  const onset = incident.onset[0];
  const parts = [
    onset ? `onset ${formatValue(metric, onset.value)}` : null,
    `peak ${formatValue(metric, incident.peak.value)}`,
    `${incident.endedMs === null ? 'last' : 'recovered'} ${formatValue(metric, incident.last.value)}`,
  ];
  return parts.filter(Boolean).join(' → ');
}

/** Local wall-clock time of an observation. */
export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString();
}
