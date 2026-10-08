// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { answerSshPrompt, onSshPrompt } from '@openrows/ipc';
import type { SshPromptAnswer, SshPromptDto } from '@openrows/ipc';

type OpenPrompt = { id: string; prompt: SshPromptDto };

type SshPrompt = {
  /** The prompt waiting for an answer, if any. */
  current: OpenPrompt | null;
  /** Send the answer to the core (never stored in the webview) and close. */
  answer: (answer: SshPromptAnswer) => void;
};

/**
 * Host-key and keyboard-interactive prompts raised by SSH connects. The core
 * opens a prompt with an event and closes it with another one when it was
 * answered, abandoned or timed out.
 */
export function useSshPrompt(): SshPrompt {
  const [current, setCurrent] = useState<OpenPrompt | null>(null);
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void onSshPrompt(({ id, prompt }) => {
      setCurrent((open) => {
        if (prompt) return { id, prompt };
        return open?.id === id ? null : open;
      });
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
  return {
    current,
    answer: (answer) => {
      if (!current) return;
      setCurrent(null);
      // A prompt that already closed (timeout) has nothing left to answer.
      answerSshPrompt(current.id, answer).catch(() => undefined);
    },
  };
}
