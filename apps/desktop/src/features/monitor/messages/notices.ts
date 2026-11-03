// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t, tn } = defineMessages({
  monitorIncidents: 'Monitor incidents',
  dismiss: 'Dismiss',
  incidentOpened: 'Incident opened',
  incidentResolved: 'Incident resolved',
  stateTitleDatabase: '{state}: {title} on {database} ({value})',
  firesFireMetric: 'fires {fire} {fireValue} · clears {clear} {clearValue}',
  samplesSamplesSeconds: '{samples} samples, {seconds} s',
  sampleCount: { one: '{count} sample', other: '{count} samples' },
  onsetValue: 'onset {value}',
  peakValue: 'peak {value}',
  lastValue: 'last {value}',
  recoveredValue: 'recovered {value}',
});
