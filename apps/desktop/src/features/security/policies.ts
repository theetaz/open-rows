// SPDX-License-Identifier: Apache-2.0
import type { ProfileDto, SafetyDto } from '@openrows/ipc';
import { t } from './messages';
/** Compact connection safety names. */
export const POLICY_LABELS: Record<SafetyDto, string> = {
  unrestricted: t('unrestricted'),
  confirmAll: t('confirmAll'),
  confirmWrites: t('confirmWrites'),
  authenticateAll: t('authenticateAll'),
  authenticateWrites: t('authenticateWrites'),
  readOnly: 'Read-only',
};
/** One-line descriptions shown in the connection form. */
export const POLICY_DETAILS: Record<SafetyDto, string> = {
  unrestricted: t('runOrdinaryStatements'),
  confirmAll: t('confirmEveryStatement'),
  confirmWrites: t('confirmOperationsMay'),
  authenticateAll: t('useOsAuthentication'),
  authenticateWrites: t('useOsAuthenticationEvery'),
  readOnly: t('rejectEveryOperation'),
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
      label: t('readOnlyFile'),
      detail: t('databaseFileOpen'),
    };
  return {
    on: profile.safety !== 'unrestricted',
    label: POLICY_LABELS[profile.safety],
    detail: POLICY_DETAILS[profile.safety],
  };
}
