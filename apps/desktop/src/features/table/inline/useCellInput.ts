// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { editorKey } from './editorKey';
import { temporalKind } from './temporal';
import type { TemporalKind } from './temporal';
import type { InlineEditor, OpenEditor } from './useInlineEditor';

type Context = { open: OpenEditor; editor: InlineEditor };
type CellInput = {
  inputRef: React.RefObject<HTMLInputElement | null>;
  kind: TemporalKind | null;
  handlers: Pick<
    React.InputHTMLAttributes<HTMLInputElement>,
    'onFocus' | 'onBlur' | 'onKeyDown'
  >;
  openerRef: React.RefObject<HTMLButtonElement | null>;
  onOpen: () => void;
  onPick: (text: string | null) => void;
};

/** Typed entry owns focus and staging; opening a system picker never commits. */
export function useCellInput(context: Context): CellInput {
  const inputRef = useRef<HTMLInputElement>(null);
  const openerRef = useRef<HTMLButtonElement>(null);
  const pickingRef = useRef(false);
  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus({ preventScroll: true });
    input.setSelectionRange(input.value.length, input.value.length);
  }, []);
  const { open, editor } = context;
  const kind = open.ready ? temporalKind(open.edit.dataType) : null;
  const pick = open.pick === true && kind !== null;
  useEffect(() => {
    if (pick) openerRef.current?.click();
  }, [pick]);
  return {
    inputRef,
    kind,
    handlers: {
      onFocus: () => {
        pickingRef.current = false;
      },
      onBlur: (event) => {
        blurInput(event, { editor, pickingRef });
      },
      onKeyDown: (event) => {
        cellKey(event, context, { kind, openerRef });
      },
    },
    openerRef,
    onOpen: () => {
      pickingRef.current = true;
    },
    onPick: (text) => {
      if (text !== null) editor.change(text);
      inputRef.current?.focus({ preventScroll: true });
    },
  };
}

function blurInput(
  event: React.FocusEvent<HTMLInputElement>,
  state: {
    editor: InlineEditor;
    pickingRef: React.RefObject<boolean>;
  },
): void {
  const { pickingRef, editor } = state;
  const target = event.relatedTarget;
  if (target instanceof Element && target.closest('[data-temporal-picker]')) {
    pickingRef.current = true;
    return;
  }
  if (!pickingRef.current) editor.commit(event.currentTarget.value);
}

function cellKey(
  event: React.KeyboardEvent<HTMLInputElement>,
  context: Context,
  picker: {
    kind: TemporalKind | null;
    openerRef: React.RefObject<HTMLButtonElement | null>;
  },
): void {
  const { kind, openerRef } = picker;
  if (
    kind &&
    event.altKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.nativeEvent.isComposing &&
    event.key === 'ArrowDown'
  ) {
    event.preventDefault();
    event.stopPropagation();
    // The picker mounts only once chosen; ⌥↓ chooses it like the stepper menu.
    if (openerRef.current) openerRef.current.click();
    else context.editor.manual(true);
  } else editorKey(event, context, false);
}
