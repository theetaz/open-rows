// SPDX-License-Identifier: Apache-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, RefObject } from 'react';
import { material } from '../../tokens/material';
import { useReducedMotion } from '../useReducedMotion';

/** The chosen row blinks off and on before the menu fades. */
const BLINK = 120;
/** Type-ahead letters are forgotten after this pause. */
const TYPEAHEAD_RESET = 600;

/**
 * Closing: `choose` blinks the row, fades the menu, closes it, then runs the
 * command; `close` only fades and closes. Reduced motion skips both pauses.
 */
export function useMenuExit(onClose: () => void): {
  chosen: { level: number; index: number } | null;
  closing: boolean;
  choose: (level: number, index: number, run: () => void) => void;
  close: () => void;
} {
  const reduced = useReducedMotion();
  const [chosen, setChosen] = useState<{ level: number; index: number } | null>(
    null,
  );
  const [closing, setClosing] = useState(false);
  const timers = useRef<number[]>([]);
  useEffect(
    () => () => {
      timers.current.forEach((timer) => {
        window.clearTimeout(timer);
      });
    },
    [],
  );
  const later = useCallback((run: () => void, delay: number) => {
    timers.current.push(window.setTimeout(run, delay));
  }, []);
  const close = useCallback(
    (run?: () => void) => {
      const finish = (): void => {
        onClose();
        run?.();
      };
      if (reduced) {
        finish();
        return;
      }
      setClosing(true);
      later(finish, parseFloat(material['motion-quick']));
    },
    [onClose, reduced, later],
  );
  const choose = useCallback(
    (level: number, index: number, run: () => void) => {
      if (reduced) {
        close(run);
        return;
      }
      setChosen({ level, index });
      later(() => {
        close(run);
      }, BLINK);
    },
    [close, reduced, later],
  );
  return {
    chosen,
    closing,
    choose,
    close: () => {
      close();
    },
  };
}

/** Clicks outside, a resize or leaving the window close the menu at once. */
export function useMenuDismiss(
  container: RefObject<HTMLElement | null>,
  onClose: () => void,
): void {
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  }, [onClose]);
  useEffect(() => {
    const opener = document.activeElement;
    const dismiss = (): void => {
      close.current();
    };
    const outside = (event: Event): void => {
      const target = event.target;
      if (target instanceof Node && !container.current?.contains(target))
        dismiss();
    };
    document.addEventListener('pointerdown', outside, true);
    window.addEventListener('resize', dismiss);
    window.addEventListener('blur', dismiss);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('blur', dismiss);
      restoreFocus(opener);
    };
  }, [container]);
}

/** Give focus back to the opener when the menu took it with it. */
function restoreFocus(opener: Element | null): void {
  const focus = document.activeElement;
  const lost = !focus || focus === document.body || !focus.isConnected;
  if (opener instanceof HTMLElement && opener.isConnected && lost)
    opener.focus();
}

/** Letters typed in quick succession, for type-ahead; '' for other keys. */
export function useTypeahead(): (event: KeyboardEvent) => string {
  const typed = useRef({ text: '', at: 0 });
  return useCallback((event: KeyboardEvent) => {
    const letter =
      event.key.length === 1 &&
      event.key !== ' ' &&
      !event.metaKey &&
      !event.ctrlKey;
    if (!letter) return '';
    const now = event.timeStamp;
    const fresh = now - typed.current.at > TYPEAHEAD_RESET;
    typed.current = {
      text: (fresh ? '' : typed.current.text) + event.key,
      at: now,
    };
    return typed.current.text;
  }, []);
}
