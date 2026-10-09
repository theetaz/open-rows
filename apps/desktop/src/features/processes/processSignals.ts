// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { confirmDestructive, signalProcess } from '@openrows/ipc';
import type {
  DestructivePrompt,
  ProcessDto,
  ProcessSignalDto,
} from '@openrows/ipc';
import { processLabel } from './processModel';

/** A signal waiting for the in-app confirmation sheet. */
export type PendingSignal = { process: ProcessDto; signal: ProcessSignalDto };

/**
 * The confirmation text: what the signal does, and a clear warning when the
 * process is one of the app's own connections or the one the list runs on.
 */
export function signalPrompt({
  process,
  signal,
}: PendingSignal): DestructivePrompt {
  const label = processLabel(process);
  const cancel = signal === 'cancelQuery';
  const effect = cancel
    ? 'Its running statement stops with an error; the session stays connected.'
    : 'Its session is disconnected and any open transaction is rolled back.';
  const own = process.current
    ? ' This is the connection this process list runs on; the list reconnects on its next refresh.'
    : process.app
      ? ' This is one of OpenRows’ own connections, possibly a query tab of yours.'
      : '';
  return {
    title: cancel ? `Cancel the query of ${label}?` : `Kill ${label}?`,
    message: effect + own,
    action: cancel ? 'Cancel Query' : 'Kill Process',
  };
}

/** Confirm-then-send for Cancel Query and Kill Process. */
type ProcessSignals = {
  /** Ask, then signal; refused up front on read-only connections. */
  request: (process: ProcessDto, signal: ProcessSignalDto) => void;
  /** The signal the fallback sheet asks about (no native alert). */
  pending: PendingSignal | null;
  confirm: () => void;
  dismiss: () => void;
  /** The last refusal or failure; `null` after a success. */
  error: string | null;
};

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/**
 * Signals for one connection and database. Every signal is confirmed (a
 * native alert, else the in-app sheet); only a confirmed signal at the
 * list's own connection may end it (`allowOwn`).
 */
export function useProcessSignals(input: {
  profileId: string;
  database: string;
  readOnly: boolean;
  onSignalled: () => void;
}): ProcessSignals {
  const [pending, setPending] = useState<PendingSignal | null>(null);
  const [error, setError] = useState<string | null>(null);
  const send = ({ process, signal }: PendingSignal): void => {
    signalProcess({
      profileId: input.profileId,
      database: input.database,
      processId: process.id,
      signal,
      allowOwn: process.current,
    }).then(
      () => {
        setError(null);
        input.onSignalled();
      },
      (failure: unknown) => {
        setError(message(failure));
      },
    );
  };
  const request = (process: ProcessDto, signal: ProcessSignalDto): void => {
    if (input.readOnly) {
      setError(
        'This connection is read-only: cancelling queries and killing processes are disabled',
      );
      return;
    }
    const asked = { process, signal };
    const fallback = (): void => {
      setPending(asked);
    };
    confirmDestructive(signalPrompt(asked)).then((answer) => {
      if (answer === null) fallback();
      else if (answer) send(asked);
    }, fallback);
  };
  return {
    request,
    pending,
    confirm: () => {
      if (pending) send(pending);
      setPending(null);
    },
    dismiss: () => {
      setPending(null);
    },
    error,
  };
}
