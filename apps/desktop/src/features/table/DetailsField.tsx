// SPDX-License-Identifier: Apache-2.0
import { useCallback, useRef, useState } from 'react';
import { booleanText, Icon, IconButton, TextArea } from '@openrows/ui';
import { editableText, stagedDisplay, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import { fieldActions } from './fieldActions';
import { FieldMenu } from './FieldMenu';
import type { FieldInspect } from './FieldMenu';
import { isBooleanType } from './inline/cellKind';
import { ValueInspector } from '../value/ValueInspector';
import type { TableColumnDto } from '@openrows/ipc';
import type { RowEditor, RowField, RowSource } from './row';
import styles from './RowDetails.module.css';
import { t } from './messages/initial';

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
 * One inspector field row: name and type on the left, the value box and its
 * value menu on the right; the menu also opens the exact full value under
 * the row. Typing then blur or Enter (⌘Enter once the value has several
 * lines) stages the text; Escape reverts the typing.
 */
export function DetailsField({
  field,
  editor,
  source,
}: {
  field: RowField;
  editor: RowEditor | undefined;
  /** The stored result the row came from; enables the full-value inspector. */
  source?: RowSource | undefined;
}): React.JSX.Element {
  const { column, locked, editable } = access(field, editor);
  const shown = shownOf(field, isBoolean(field, column));
  const base = `${field.state}\u0000${shown.marker}\u0000${shown.text}`;
  const [draft, setDraft] = useState<Draft | null>(null);
  const inspect = useInspect(source);
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
    <div className={styles.field} data-state={field.state} data-menu-row>
      <div className={styles.label}>
        <span>{field.name}</span>
        <small title={field.type ?? undefined}>{field.type}</small>
      </div>
      <TextArea
        controlSize="box"
        aria-label={t('valueName', { name: field.name })}
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
          editor && column ? (
            <ValueMenu
              field={field}
              editor={editor}
              actions={fieldActions(
                column,
                locked || !editor.ready,
                current ? textValue(current.text) : (field.value ?? null),
              )}
              onValue={stage}
              inspect={inspect}
            />
          ) : (
            inspect && <FullValueButton name={field.name} inspect={inspect} />
          )
        }
      />
      {source && inspect?.open && (
        <FieldInspection field={field} source={source} />
      )}
    </div>
  );
}

/** Whether the field's exact value is shown; only rows from a stored result offer it. */
function useInspect(source: RowSource | undefined): FieldInspect | undefined {
  const [open, setOpen] = useState(false);
  if (!source) return undefined;
  return {
    open,
    toggle: () => {
      setOpen(!open);
    },
  };
}

/** A read-only field's own affordance for its exact value (no value menu). */
function FullValueButton({
  name,
  inspect,
}: {
  name: string;
  inspect: FieldInspect;
}): React.JSX.Element {
  return (
    <IconButton
      size="sm"
      variant="ghost"
      className={styles.chevron}
      aria-label={inspect.open ? t('hideFullValue') : t('fullValue')}
      title={name}
      aria-expanded={inspect.open}
      onClick={inspect.toggle}
    >
      <Icon name="eye" />
    </IconButton>
  );
}

/** A field's exact stored value, opened from its value menu. */
function FieldInspection(props: {
  field: RowField;
  source: RowSource;
}): React.JSX.Element {
  const { field, source } = props;
  return (
    <div className={styles.full}>
      <ValueInspector
        key={`${source.resultId}:${String(source.row)}:${String(field.index)}`}
        address={{
          resultId: source.resultId,
          row: source.row,
          column: field.index,
        }}
        name={field.name}
        type={field.type}
        text={field.text}
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
  inspect: FieldInspect | undefined;
}): React.JSX.Element {
  const opener = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  // A press on the chevron while the menu is open closes it as an outside
  // press; the click that follows must not open it again.
  const closing = useRef(false);
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
        aria-label={t('valueOptionsName', { name: field.name })}
        aria-haspopup="menu"
        aria-expanded={open}
        onPointerDown={() => {
          closing.current = open;
        }}
        onMouseDown={(event) => {
          // Keep focus (and any typing) in the value box until an item is chosen.
          event.preventDefault();
        }}
        onClick={() => {
          if (closing.current) {
            closing.current = false;
            return;
          }
          if (!editor.ready) editor.load();
          setOpen(true);
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
          inspect={props.inspect}
        />
      )}
    </>
  );
}
