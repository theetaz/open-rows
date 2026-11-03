// SPDX-License-Identifier: Apache-2.0
import type { AlertRule, Incident } from '@openrows/ipc';
import { t, tn } from './messages/notices';

/** Ratios are fractions in core; show them as percentages. */
export function formatValue(metric: string, value: number | null): string {
  if (value === null) return 'unknown';
  if (metric.endsWith('_ratio')) return `${(value * 100).toFixed(1)} %`;
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

/** Firing and recovery thresholds, e.g. "fires ≥ 5 · clears ≤ 1". */
export function thresholds(rule: AlertRule): string {
  const [fire, clear] = rule.comparison === 'above' ? ['≥', '≤'] : ['≤', '≥'];
  return t('firesFireMetric', {
    fire,
    fireValue: formatValue(rule.metric, rule.fire),
    clear,
    clearValue: formatValue(rule.metric, rule.clear),
  });
}

/** Required hold as samples and seconds. */
export function hold(value: { samples: number; ms: number }): string {
  const seconds = value.ms / 1000;
  return seconds
    ? t('samplesSamplesSeconds', {
        samples: String(value.samples),
        seconds: String(seconds),
      })
    : tn('sampleCount', value.samples);
}

/** Onset, peak and last values with the thresholds that applied. */
export function evidence(incident: Incident): string {
  const metric = incident.rule.metric;
  const onset = incident.onset[0];
  const parts = [
    onset ? t('onsetValue', { value: formatValue(metric, onset.value) }) : null,
    t('peakValue', { value: formatValue(metric, incident.peak.value) }),
    incident.endedMs === null
      ? t('lastValue', { value: formatValue(metric, incident.last.value) })
      : t('recoveredValue', {
          value: formatValue(metric, incident.last.value),
        }),
  ];
  return parts.filter(Boolean).join(' → ');
}

/** Local wall-clock time of an observation. */
export function clock(ms: number | null): string {
  return ms === null ? '—' : new Date(ms).toLocaleTimeString();
}
