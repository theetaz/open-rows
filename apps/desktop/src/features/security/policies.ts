// SPDX-License-Identifier: Apache-2.0
import type { SafetyDto } from '@openrows/ipc';
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
