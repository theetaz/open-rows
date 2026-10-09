// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';

/** A revealed secret hides again after this long. */
export const REVEAL_TIMEOUT_MS = 30_000;

type RevealedSecret = {
  /** The plain value while revealed; `null` while masked. */
  value: string | null;
  /** Why the Keychain read failed, if it did. */
  error: string | null;
  busy: boolean;
  show: () => Promise<void>;
  hide: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * A saved secret read from the Keychain only when the user asks: it lives in
 * this component's state alone and is dropped on hide, after
 * [`REVEAL_TIMEOUT_MS`], and when the component unmounts (the sheet closes).
 */
export function useRevealedSecret(read: () => Promise<string>): RevealedSecret {
  const [value, setValue] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const mounted = useRef(true);
  const hide = (): void => {
    clearTimeout(timer.current);
    setValue(null);
  };
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      clearTimeout(timer.current);
    };
  }, []);
  const show = async (): Promise<void> => {
    setError(null);
    setBusy(true);
    try {
      const secret = await read();
      if (!mounted.current) return;
      setValue(secret);
      clearTimeout(timer.current);
      timer.current = setTimeout(hide, REVEAL_TIMEOUT_MS);
    } catch (failure) {
      if (mounted.current) setError(message(failure));
    } finally {
      if (mounted.current) setBusy(false);
    }
  };
  return { value, error, busy, show, hide };
}
