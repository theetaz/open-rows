// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import { parseMonitorSettings } from './monitorSettings';
it('defaults monitor history off and accepts only bounded connection UUID opt-ins', () => {
  expect(parseMonitorSettings(null)).toEqual({ enabledProfiles: [] });
  expect(
    parseMonitorSettings({
      enabledProfiles: [
        'bad',
        7,
        'AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA',
        'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
      ],
    }),
  ).toEqual({ enabledProfiles: ['aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa'] });
});
