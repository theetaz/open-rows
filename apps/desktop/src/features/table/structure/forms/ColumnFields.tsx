// SPDX-License-Identifier: Apache-2.0
import { useId } from 'react';
import { SelectField, Switch, TextField } from '@openrows/ui';
import type { DefaultDto, EngineDto, ObjectFeaturesDto } from '@openrows/ipc';
import type { ColumnDraft } from './columnDraft';
import {
  collationHints,
  encodingHints,
  identityOptions,
  typeSpecs,
} from './dialect';
import styles from './Forms.module.css';

const defaultModes: readonly { value: DefaultDto['kind']; label: string }[] = [
  { value: 'none', label: 'No default' },
  { value: 'expression', label: 'SQL expression' },
  { value: 'literal', label: 'Text literal' },
];

type Props = {
  draft: ColumnDraft;
  onChange: (draft: ColumnDraft) => void;
  engine: EngineDto;
  features: ObjectFeaturesDto;
};

/**
 * The column form: name, type with length or precision and scale,
 * nullability, default, numbering, collation, character set and comment;
 * fields the engine lacks are left out.
 */
export function ColumnFields(props: Props): React.JSX.Element {
  const { draft, onChange, features } = props;
  const set = (patch: Partial<ColumnDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  return (
    <div className={styles.grid}>
      <TextField
        label="Name"
        value={draft.name}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          set({ name: event.target.value });
        }}
      />
      <TypeFields {...props} />
      <label className={styles.switchRow}>
        <Switch
          aria-label="Allow NULL"
          checked={draft.nullable}
          onCheckedChange={(nullable) => {
            set({ nullable });
          }}
        />
        Allow NULL
      </label>
      <DefaultFields draft={draft} onChange={onChange} />
      <NumberingFields {...props} />
      {features.comments && (
        <TextField
          label="Comment"
          className={styles.wide}
          value={draft.comment}
          onChange={(event) => {
            set({ comment: event.target.value });
          }}
        />
      )}
    </div>
  );
}

/** Type (any name may be typed; the list offers the engine's common ones) and size. */
function TypeFields({ draft, onChange, engine }: Props): React.JSX.Element {
  const list = useId();
  const spec = typeSpecs(engine).find(
    (item) => item.name.toLowerCase() === draft.type.trim().toLowerCase(),
  );
  const sized = Boolean(spec?.size) || draft.length !== '';
  const precision = spec?.size === 'precision' || draft.scale !== '';
  const set = (patch: Partial<ColumnDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  return (
    <>
      <TextField
        label="Type"
        list={list}
        value={draft.type}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          set({ type: event.target.value });
        }}
      />
      <datalist id={list}>
        {typeSpecs(engine).map((item) => (
          <option key={item.name} value={item.name} />
        ))}
      </datalist>
      {sized && (
        <div className={styles.pair}>
          <TextField
            label={precision ? 'Precision' : 'Length'}
            inputMode="numeric"
            value={draft.length}
            onChange={(event) => {
              set({ length: event.target.value.replace(/\D/g, '') });
            }}
          />
          {precision && (
            <TextField
              label="Scale"
              inputMode="numeric"
              value={draft.scale}
              onChange={(event) => {
                set({ scale: event.target.value.replace(/\D/g, '') });
              }}
            />
          )}
        </div>
      )}
    </>
  );
}

function DefaultFields({
  draft,
  onChange,
}: {
  draft: ColumnDraft;
  onChange: (draft: ColumnDraft) => void;
}): React.JSX.Element {
  return (
    <div className={styles.pair}>
      <SelectField
        label="Default"
        value={draft.defaultMode}
        options={defaultModes}
        onValueChange={(defaultMode) => {
          onChange({ ...draft, defaultMode });
        }}
      />
      {draft.defaultMode !== 'none' && (
        <TextField
          aria-label={
            draft.defaultMode === 'literal'
              ? 'Default text'
              : 'Default expression'
          }
          placeholder={draft.defaultMode === 'literal' ? 'Text' : 'now()'}
          value={draft.defaultText}
          spellCheck={false}
          onChange={(event) => {
            onChange({ ...draft, defaultText: event.target.value });
          }}
        />
      )}
    </div>
  );
}

/** Identity or auto-increment, collation and character set, as supported. */
function NumberingFields({
  draft,
  onChange,
  engine,
  features,
}: Props): React.JSX.Element {
  const collations = useId();
  const charsets = useId();
  const set = (patch: Partial<ColumnDraft>): void => {
    onChange({ ...draft, ...patch });
  };
  return (
    <>
      {features.identity && (
        <SelectField
          label="Numbering"
          value={draft.identity}
          options={identityOptions(engine, features)}
          onValueChange={(identity) => {
            set({ identity });
          }}
        />
      )}
      {features.charsets && (
        <TextField
          label="Character set"
          list={charsets}
          placeholder="Table default"
          value={draft.charset}
          onChange={(event) => {
            set({ charset: event.target.value });
          }}
        />
      )}
      {features.collations && (
        <TextField
          label="Collation"
          list={collations}
          placeholder="Default"
          value={draft.collation}
          onChange={(event) => {
            set({ collation: event.target.value });
          }}
        />
      )}
      <datalist id={charsets}>
        {encodingHints(engine).map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
      <datalist id={collations}>
        {collationHints(engine).map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
    </>
  );
}
