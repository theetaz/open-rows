// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { parsePostgresUrl } from '@openrows/ipc';
import { withPassword, withUrl } from './form';
import type { Draft } from './form';
import { composeUrl, splitUrl } from './urlText';
import type { UrlPassword } from './urlText';

/** Typing pauses this long before the URL is parsed; paste parses at once. */
const URL_PARSE_DELAY_MS = 250;

export type UrlField = {
  /** The URL shown: what the user typed (password masked) or the fields' URL. */
  value: string;
  error: string | null;
  pending: boolean;
  edit: (text: string, immediate: boolean) => void;
  flush: () => void;
  /** A field changed: show the URL derived from the fields again. */
  release: () => void;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

function applyPassword(draft: Draft, password: UrlPassword): Draft {
  if (password.kind === 'typed') return withPassword(draft, password.value);
  if (password.kind === 'none' && draft.passwordMode === 'set')
    return withPassword(draft, '');
  return draft;
}

type Sink = {
  ticket: { current: number };
  setDraft: Dispatch<SetStateAction<Draft>>;
  setError: (error: string | null) => void;
};

/** Parse in the core; a newer edit or a field change discards stale results. */
function parse(stripped: string, sink: Sink): void {
  const current = ++sink.ticket.current;
  parsePostgresUrl(stripped).then(
    (parsed) => {
      if (current !== sink.ticket.current) return;
      sink.setDraft((value) => withUrl(value, parsed));
      sink.setError(null);
    },
    (failure: unknown) => {
      if (current === sink.ticket.current) sink.setError(message(failure));
    },
  );
}

/** Two-way URL ↔ fields sync; the core parses a password-free copy of the URL. */
export function useUrlField(
  draft: Draft,
  setDraft: Dispatch<SetStateAction<Draft>>,
): UrlField {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [queued, setQueued] = useState<string | null>(null);
  const ticket = useRef(0);
  useEffect(() => {
    if (queued === null) return;
    const timer = setTimeout(() => {
      setQueued(null);
      parse(queued, { ticket, setDraft, setError });
    }, URL_PARSE_DELAY_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [queued, setDraft]);
  const now = (stripped: string): void => {
    setQueued(null);
    parse(stripped, { ticket, setDraft, setError });
  };
  return {
    value: text ?? composeUrl(draft),
    error,
    pending: queued !== null,
    edit: (raw, immediate) => {
      const split = splitUrl(raw);
      setText(split.masked);
      setDraft((value) => applyPassword(value, split.password));
      if (split.password.kind === 'invalid')
        setError('The password is not valid percent-encoding');
      else if (!raw.trim()) setQueued(null);
      else if (immediate) now(split.stripped);
      else setQueued(split.stripped);
    },
    flush: () => {
      if (queued !== null) now(queued);
    },
    release: () => {
      ticket.current += 1;
      setText(null);
      setQueued(null);
      setError(null);
    },
  };
}
