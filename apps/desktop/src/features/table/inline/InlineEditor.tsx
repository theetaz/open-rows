// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { PointMenu } from '../../../shell/PointMenu';
import { ValuePopover } from './ValuePopover';
import { editorKey } from './editorKey';
import type { InlineEditor, OpenEditor } from './useInlineEditor';
import styles from './Inline.module.css';

/** The open in-place editor, if any: input, popover or value menu. */
export function InlineEditorView({
  editor,
}: {
  editor: InlineEditor;
}): React.JSX.Element | null {
  const open = editor.open;
  if (!open) return null;
  if (open.mode === 'menu') return <ValueMenu open={open} editor={editor} />;
  if (open.mode === 'popover')
    return <ValuePopover open={open} editor={editor} />;
  return <CellInput open={open} editor={editor} />;
}

type Props = { open: OpenEditor; editor: InlineEditor };

/** A one-line input drawn exactly over the cell, in the grid's font. */
function CellInput({ open, editor }: Props): React.JSX.Element {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    input.focus({ preventScroll: true });
    input.setSelectionRange(input.value.length, input.value.length);
  }, []);
  return (
    <input
      ref={ref}
      className={styles.cellInput}
      aria-label={`Edit ${open.edit.name}`}
      spellCheck={false}
      autoComplete="off"
      readOnly={!open.ready}
      value={open.value}
      style={{
        left: open.rect.x,
        top: open.rect.y,
        width: open.rect.width,
        height: open.rect.height,
        font: open.font,
      }}
      onChange={(event) => {
        editor.change(event.target.value);
      }}
      onKeyDown={(event) => {
        editorKey(event, { open, editor }, false);
      }}
      onBlur={(event) => {
        editor.commit(event.currentTarget.value);
      }}
    />
  );
}

/** TRUE/FALSE or enum labels, plus NULL for nullable columns. */
function ValueMenu({ open, editor }: Props): React.JSX.Element {
  return (
    <PointMenu
      x={open.anchor.x}
      y={open.anchor.y}
      label={`Values for ${open.edit.name}`}
      onClose={editor.cancel}
    >
      {open.edit.options?.map((option) => (
        <button
          type="button"
          role="menuitem"
          key={option.label}
          onClick={() => {
            editor.commit(option.value);
          }}
        >
          {option.label}
        </button>
      ))}
    </PointMenu>
  );
}
