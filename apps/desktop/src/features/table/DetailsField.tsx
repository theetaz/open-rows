// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import { booleanText } from '@openrows/grid';
import { Icon, IconButton, TextArea } from '@openrows/ui';
import { editableText, stagedDisplay, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import { fieldActions } from './fieldActions';
import { FieldMenu } from './FieldMenu';
import { isBooleanType } from './inline/cellKind';
import type { TableColumnDto } from '@openrows/ipc';
import type { RowEditor, RowField } from './row';
import styles from './RowDetails.module.css';

/** Typing over the value `base` showed; `staged` once handed to the staging API. */
type Draft = { base: string; text: string; staged: boolean };

/** Booleans read TRUE/FALSE as in the grid; the stored text itself is never changed. */
function isBoolean(
  field: RowField,
  column: TableColumnDto | undefined,
): boolean {
  return (
    isBooleanType(field.type ?? '') ||
    (column !== undefined && isBooleanType(column.dataType))
  );
}

/** What the value box shows: editable text, and a muted marker when empty. */
function shownOf(
  field: RowField,
  isBool: boolean,
): { text: string; marker: string } {
  const show = (text: string): string => (isBool ? booleanText(text) : text);
  if (field.value) {
    const display = stagedDisplay(field.value);
    return {
      text: show(editableText(field.value)),
      marker: display.placeholder ? display.text : '',
    };
  }
  if (field.state === 'null') return { text: '', marker: 'NULL' };
  if (field.state === 'empty') return { text: '', marker: 'EMPTY' };
  return { text: show(field.text), marker: '' };
}

/** Whether the field can take values: an editable table, a writable column, a live row. */
function access(
  field: RowField,
  editor: RowEditor | undefined,
): { column: TableColumnDto | undefined; locked: boolean; editable: boolean } {
  const column = editor?.columns.find((item) => item.name === field.name);
  const locked =
    !editor ||
    !column ||
    field.state === 'deleted' ||
    editor.protected.includes(field.name);
  return {
    column,
    locked,
    editable: !locked && editor.ready && field.value !== undefined,
  };
}

/**
 * One inspector field: name, type, the value box and its value menu. Typing
 * then blur or Enter (⌘Enter once the value has several lines) stages the
 * text; Escape reverts the typing.
 */
export function DetailsField({
  field,
  editor,
}: {
  field: RowField;
  editor: RowEditor | undefined;
}): React.JSX.Element {
  const { column, locked, editable } = access(field, editor);
  const shown = shownOf(field, isBoolean(field, column));
  const base = `${field.state}\u0000${shown.marker}\u0000${shown.text}`;
  const [draft, setDraft] = useState<Draft | null>(null);
  // A newly staged or reloaded value replaces any typing that produced it.
  const current = draft?.base === base ? draft : null;
  const text = current?.text ?? shown.text;
  const stage = (value: CellValue): void => {
    editor?.stage(field.name, value);
  };
  const commit = (): void => {
    if (!current || current.staged || !editable) return;
    stage(textValue(current.text));
    setDraft({ ...current, staged: true });
  };
  return (
    <div className={styles.field} data-state={field.state}>
      <div className={styles.label}>
        <span>{field.name}</span>
        <small title={field.type ?? undefined}>{field.type}</small>
      </div>
      <TextArea
        aria-label={`Value of ${field.name}`}
        className={styles.value}
        value={text}
        placeholder={shown.marker}
        readOnly={!editable}
        spellCheck={false}
        onFocus={() => {
          if (!locked && !editable) editor?.load();
        }}
        onChange={(event) => {
          setDraft({ base, text: event.target.value, staged: false });
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          valueKey(event, text, {
            commit,
            revert: () => {
              setDraft(null);
            },
          });
        }}
        trailing={
          editor &&
          column && (
            <ValueMenu
              field={field}
              editor={editor}
              actions={fieldActions(
                column,
                locked || !editor.ready,
                current ? textValue(current.text) : (field.value ?? null),
              )}
              onValue={stage}
            />
          )
        }
      />
    </div>
  );
}

/** Enter (⌘Enter when multi-line) stages; Escape reverts; nothing else is intercepted. */
function valueKey(
  event: React.KeyboardEvent<HTMLTextAreaElement>,
  text: string,
  handlers: { commit: () => void; revert: () => void },
): void {
  if (event.nativeEvent.isComposing) return;
  if (event.key === 'Escape') {
    event.preventDefault();
    event.stopPropagation();
    handlers.revert();
    return;
  }
  if (event.key !== 'Enter' || event.shiftKey || event.altKey) return;
  if (text.includes('\n') && !event.metaKey) return;
  event.preventDefault();
  handlers.commit();
}

function ValueMenu(props: {
  field: RowField;
  editor: RowEditor;
  actions: ReturnType<typeof fieldActions>;
  onValue: (value: CellValue) => void;
}): React.JSX.Element {
  const opener = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => {
    setOpen(false);
    opener.current?.focus();
  }, []);
  const { field, editor } = props;
  return (
    <>
      <IconButton
        ref={opener}
        size="sm"
        variant="ghost"
        className={styles.chevron}
        aria-label={`Value options for ${field.name}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onMouseDown={(event) => {
          // Keep focus (and any typing) in the value box until an item is chosen.
          event.preventDefault();
        }}
        onClick={() => {
          if (!editor.ready) editor.load();
          setOpen(!open);
        }}
      >
        <Icon name="chevronDown" />
      </IconButton>
      {open && (
        <FieldMenu
          name={field.name}
          actions={props.actions}
          opener={opener}
          onValue={props.onValue}
          onFile={() => {
            editor.addFile(field.name);
          }}
          onClose={close}
        />
      )}
    </>
  );
}
