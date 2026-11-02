// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto, SafetyDto } from '@openrows/ipc';
/** Compact connection safety names. */
export const POLICY_LABELS: Record<SafetyDto, string> = {
  unrestricted: 'Unrestricted',
  confirmAll: 'Confirm all',
  confirmWrites: 'Confirm writes',
  authenticateAll: 'Authenticate all',
  authenticateWrites: 'Authenticate writes',
  readOnly: 'Read-only',
};
/** One-line descriptions shown in the connection form. */
export const POLICY_DETAILS: Record<SafetyDto, string> = {
  unrestricted:
    'Run ordinary statements without prompts; dangerous statements still confirm.',
  confirmAll: 'Confirm every statement, including reads.',
  confirmWrites:
    'Confirm operations that may change data; default for production.',
  authenticateAll: 'Use OS authentication for every statement.',
  authenticateWrites: 'Use OS authentication for every non-read statement.',
  readOnly: 'Reject every operation that may change data.',
};

/** The protection a connection really has, as the toolbar reports it. */
export type SafeModeState = {
  /** Anything beyond the dangerous-statement confirmation every policy keeps. */
  on: boolean;
  label: string;
  detail: string;
};

/** Read from the saved policy, the only thing enforcement looks at (FR-CONN-07). */
export function safeModeState(profile: ProfileDto): SafeModeState {
  if (profile.safety === 'unrestricted' && profile.file?.readOnly)
    return {
      on: true,
      label: 'Read-only file',
      detail:
        'The database file is open read-only, so the engine refuses writes.',
    };
  return {
    on: profile.safety !== 'unrestricted',
    label: POLICY_LABELS[profile.safety],
    detail: POLICY_DETAILS[profile.safety],
  };
}
