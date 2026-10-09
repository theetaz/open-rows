// SPDX-License-Identifier: Apache-2.0
import { useRef, useState } from 'react';
import type { ProfileDto } from '@openrows/ipc';

/** An open "enter the password" sheet for an "ask every time" connection. */
export type PasswordRequest = {
  profile: ProfileDto;
  submit: (password: string) => void;
  cancel: () => void;
};

type PasswordPrompt = {
  request: PasswordRequest | null;
  /**
   * The password for connecting `profileId`: `undefined` when the profile
   * keeps it in the Keychain, `null` when the user cancelled the sheet.
   */
  ask: (profileId: string) => Promise<string | null | undefined>;
};

/**
 * Asks for the password of "ask every time" connections in a sheet. The
 * typed value goes straight to the connect call and is not kept here. A new
 * request cancels one still open, so no connect waits forever.
 */
export function usePasswordPrompt(
  profiles: readonly ProfileDto[],
): PasswordPrompt {
  const [request, setRequest] = useState<PasswordRequest | null>(null);
  const pending = useRef<((password: null) => void) | null>(null);
  const ask = (profileId: string): Promise<string | null | undefined> => {
    const profile = profiles.find((item) => item.id === profileId);
    if (profile?.passwordStorage !== 'ask') return Promise.resolve(undefined);
    pending.current?.(null);
    return new Promise((resolve) => {
      const settle = (password: string | null): void => {
        pending.current = null;
        setRequest(null);
        resolve(password);
      };
      pending.current = resolve;
      setRequest({
        profile,
        submit: settle,
        cancel: () => {
          settle(null);
        },
      });
    });
  };
  return { request, ask };
}
