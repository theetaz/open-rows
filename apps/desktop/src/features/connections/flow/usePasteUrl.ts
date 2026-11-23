// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { parseConnectionUrl } from '@openrows/ipc';
import type { UrlDraft } from '@openrows/ipc';
import { splitUrl } from '../urlText';
import type { UrlPassword } from '../urlText';

/** A typed URL is read after this pause; a paste at once. */
const PASTE_PARSE_DELAY_MS = 200;

export type PasteUrl = {
  /** What the field shows: any password as the mask. */
  text: string;
  /** The password the URL carried, kept out of the field and the parser. */
  password: UrlPassword;
  parsed: UrlDraft | null;
  /** The text is not a connection URL (yet). */
  invalid: boolean;
  edit: (text: string, immediate: boolean) => void;
};

type Parsed = { for: string; parsed: UrlDraft | null };

/** Parse `stripped` in the core after the pause; a newer text discards the result. */
function useParsed(stripped: string, immediate: boolean): Parsed | null {
  const [result, setResult] = useState<Parsed | null>(null);
  useEffect(() => {
    if (!stripped.trim()) return;
    let live = true;
    const timer = setTimeout(
      () => {
        parseConnectionUrl(stripped).then(
          (parsed) => {
            if (live) setResult({ for: stripped, parsed });
          },
          () => {
            if (live) setResult({ for: stripped, parsed: null });
          },
        );
      },
      immediate ? 0 : PASTE_PARSE_DELAY_MS,
    );
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [stripped, immediate]);
  return result?.for === stripped ? result : null;
}

/** The engine step's paste field: the core parses a password-free copy as you type. */
export function usePasteUrl(): PasteUrl {
  const [state, setState] = useState({
    text: '',
    stripped: '',
    immediate: false,
    password: { kind: 'none' } as UrlPassword,
  });
  const current = useParsed(state.stripped, state.immediate);
  const blank = !state.stripped.trim();
  return {
    text: state.text,
    password: state.password,
    parsed: blank ? null : (current?.parsed ?? null),
    invalid: !blank && current !== null && current.parsed === null,
    edit: (text, immediate) => {
      const split = splitUrl(text);
      setState((previous) => ({
        text: split.masked,
        stripped: split.password.kind === 'invalid' ? '' : split.stripped,
        immediate,
        password:
          split.password.kind === 'masked' ? previous.password : split.password,
      }));
    },
  };
}
