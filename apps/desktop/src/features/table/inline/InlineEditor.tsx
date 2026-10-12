// SPDX-License-Identifier: Apache-2.0
import { PointMenu } from '../../../shell/PointMenu';
import { ValuePopover } from './ValuePopover';
import { useCellInput } from './useCellInput';
import type { TemporalKind } from './temporal';
import { TemporalPicker } from './TemporalPicker';
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

/**
 * A one-line input drawn exactly over the cell, in the grid's font; date and
 * time columns add a calendar button at the cell's end (FR-DATA-06).
 */
function CellInput({ open, editor }: Props): React.JSX.Element {
  const { inputRef, openerRef, kind, handlers, onOpen, onPick } = useCellInput({
    open,
    editor,
  });
  const { rect } = open;
  return (
    <>
      <input
        ref={inputRef}
        {...handlers}
        className={styles.cellInput}
        aria-label={`Edit ${open.edit.name}`}
        aria-keyshortcuts={kind ? 'Alt+ArrowDown' : undefined}
        spellCheck={false}
        autoComplete="off"
        readOnly={!open.ready}
        value={open.value}
        style={{
          left: rect.x,
          top: rect.y,
          width: rect.width,
          height: rect.height,
          font: open.font,
        }}
        onChange={(event) => {
          editor.change(event.target.value);
        }}
      />
      {kind && (
        <CellPicker
          open={open}
          kind={kind}
          openerRef={openerRef}
          onOpen={onOpen}
          onPick={onPick}
        />
      )}
    </>
  );
}

/** Calendar overlay and its return to typed entry after picking or cancellation. */
function CellPicker({
  open,
  kind,
  openerRef,
  onOpen,
  onPick,
}: {
  open: OpenEditor;
  kind: TemporalKind;
  openerRef: React.RefObject<HTMLButtonElement | null>;
  onOpen: () => void;
  onPick: (text: string | null) => void;
}): React.JSX.Element {
  const { rect } = open;
  return (
    <TemporalPicker
      kind={kind}
      openerRef={openerRef}
      value={open.value}
      name={open.edit.name}
      className={styles.cellPicker}
      style={{
        left: rect.x + rect.width,
        top: rect.y,
        height: rect.height,
      }}
      onOpen={onOpen}
      onPick={onPick}
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
