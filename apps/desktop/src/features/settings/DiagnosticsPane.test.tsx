// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { DIAGNOSTICS_KEY, parseDiagnostics } from './diagnostics';
import { DiagnosticsPane } from './DiagnosticsPane';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

it('starts with crash reports off and stores a chosen log level', async () => {
  const user = userEvent.setup();
  render(<DiagnosticsPane />);
  expect(
    screen.getByRole('switch', { name: 'Send crash reports' }),
  ).not.toBeChecked();
  await user.click(screen.getByRole('radio', { name: 'Debug' }));
  const stored = JSON.parse(
    localStorage.getItem(DIAGNOSTICS_KEY) ?? 'null',
  ) as unknown;
  expect(parseDiagnostics(stored)).toEqual({
    logLevel: 'debug',
    crashReports: false,
  });
});

it('keeps only valid diagnostics fields and falls back to the defaults', () => {
  expect(parseDiagnostics(null)).toEqual({
    logLevel: 'info',
    crashReports: false,
  });
  expect(parseDiagnostics({ logLevel: 'trace', crashReports: true })).toEqual({
    logLevel: 'trace',
    crashReports: true,
  });
  expect(
    parseDiagnostics({ logLevel: 'verbose', crashReports: 'yes' }),
  ).toEqual({ logLevel: 'info', crashReports: false });
});
