// SPDX-License-Identifier: Apache-2.0
import type { CredentialCommandDto, ProfileDto } from '@openrows/ipc';
import type { Draft, DraftErrors } from './form';
import { t } from './messages/profile';
/** JSON array preserves spaces, empty arguments and metacharacters literally. */
export function commandArguments(text: string): string[] | null {
  try {
    const value: unknown = JSON.parse(text);
    return Array.isArray(value) &&
      value.every((arg: unknown) => typeof arg === 'string')
      ? value
      : null;
  } catch {
    return null;
  }
}
/** Validate an absolute executable path, literal argv and bounded deadline. */
export function commandErrors(draft: Draft): DraftErrors {
  if (draft.passwordStorage !== 'command') return {};
  const errors: DraftErrors = {};
  if (
    !draft.commandProgram.startsWith('/') &&
    !/^[A-Za-z]:[\\/]/.test(draft.commandProgram)
  )
    errors.commandProgram = t('useAbsoluteProgram');
  if (!commandArguments(draft.commandArgs))
    errors.commandArgs = t('useJsonArray');
  const seconds = Number(draft.commandTimeout);
  if (!Number.isInteger(seconds) || seconds < 1 || seconds > 120)
    errors.commandTimeout = t('timeoutMust1');
  return errors;
}
/** Non-secret configuration; switching policy clears any stored command. */
export function commandFields(draft: Draft): CredentialCommandDto | null {
  if (draft.passwordStorage !== 'command') return null;
  return {
    program: draft.commandProgram,
    args: commandArguments(draft.commandArgs) ?? [],
    timeoutSeconds: Number(draft.commandTimeout),
  };
}

/** Restore only non-secret command fields into an editable draft. */
export function commandDraft(
  profile: ProfileDto,
): Pick<Draft, 'commandProgram' | 'commandArgs' | 'commandTimeout'> {
  const command = profile.credentialCommand;
  return {
    commandProgram: command?.program ?? '',
    commandArgs: JSON.stringify(command?.args ?? []),
    commandTimeout: String(command?.timeoutSeconds ?? 10),
  };
}
