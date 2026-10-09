// SPDX-License-Identifier: Apache-2.0
import { SelectField, TextField } from '@openrows/ui';
import type { EditInfoDto, ImportColumnDto } from '@openrows/ipc';
import { categoryLabel } from './choices';
import type { ImportDraft, NewColumn, SourceChoice } from './model';
import styles from './Import.module.css';

type Change = (patch: Partial<ImportDraft>) => void;

/** Source choice as a select value: `column:3`, `skip`, `null`, `constant`. */
const choiceValue = (choice: SourceChoice | undefined): string =>
  choice?.kind === 'column'
    ? `column:${String(choice.index)}`
    : (choice?.kind ?? 'skip');

function choiceOf(
  value: string,
  previous: SourceChoice | undefined,
): SourceChoice {
  if (value.startsWith('column:'))
    return { kind: 'column', index: Number(value.slice('column:'.length)) };
  if (value === 'constant')
    return {
      kind: 'constant',
      value: previous?.kind === 'constant' ? previous.value : '',
    };
  return value === 'null' ? { kind: 'null' } : { kind: 'skip' };
}

/** Existing table: each target column takes a source column, a constant, NULL or its default. */
export function MappingTable(props: {
  draft: ImportDraft;
  table: EditInfoDto;
  sources: readonly ImportColumnDto[];
  onChange: Change;
}): React.JSX.Element {
  const { draft, table, onChange } = props;
  const sourceOptions = [
    { value: 'skip', label: 'Skip (column default)' },
    { value: 'null', label: 'NULL' },
    { value: 'constant', label: 'Constant…' },
    ...props.sources.map((source, index) => ({
      value: `column:${String(index)}`,
      label: source.name,
    })),
  ];
  const setChoice = (column: string, choice: SourceChoice): void => {
    onChange({ mapping: { ...draft.mapping, [column]: choice } });
  };
  const toggleKey = (column: string, on: boolean): void => {
    const key = draft.key.filter((name) => name !== column);
    onChange({ key: on ? [...key, column] : key });
  };
  return (
    <table className={styles.mapping} aria-label="Column mapping">
      <thead>
        <tr>
          <th>Column</th>
          <th>Type</th>
          <th>Source</th>
          <th>Key</th>
        </tr>
      </thead>
      <tbody>
        {table.columns.map((column) => (
          <MappingRow
            key={column.name}
            name={column.name}
            dataType={column.dataType}
            generated={table.protected.includes(column.name)}
            choice={draft.mapping[column.name]}
            options={sourceOptions}
            isKey={draft.key.includes(column.name)}
            onChoice={(choice) => {
              setChoice(column.name, choice);
            }}
            onKey={(on) => {
              toggleKey(column.name, on);
            }}
          />
        ))}
      </tbody>
    </table>
  );
}

function MappingRow(props: {
  name: string;
  dataType: string;
  generated: boolean;
  choice: SourceChoice | undefined;
  options: readonly { value: string; label: string }[];
  isKey: boolean;
  onChoice: (choice: SourceChoice) => void;
  onKey: (on: boolean) => void;
}): React.JSX.Element {
  const { name, choice, generated } = props;
  return (
    <tr>
      <td>{name}</td>
      <td className={styles.typeCell}>{props.dataType}</td>
      <td className={styles.sourceCell}>
        <SelectField
          aria-label={`Source of ${name}`}
          controlSize="compact"
          disabled={generated}
          value={generated ? 'skip' : choiceValue(choice)}
          options={props.options}
          onValueChange={(value) => {
            props.onChoice(choiceOf(value, choice));
          }}
        />
        {choice?.kind === 'constant' && (
          <TextField
            aria-label={`Constant for ${name}`}
            controlSize="compact"
            value={choice.value}
            onChange={(event) => {
              props.onChoice({ kind: 'constant', value: event.target.value });
            }}
          />
        )}
      </td>
      <td>
        <input
          type="checkbox"
          aria-label={`${name} identifies rows`}
          checked={props.isKey}
          onChange={(event) => {
            props.onKey(event.target.checked);
          }}
        />
      </td>
    </tr>
  );
}

/** New table: one column per source column, renamed, retyped or left out. */
export function NewColumnsTable(props: {
  draft: ImportDraft;
  sources: readonly ImportColumnDto[];
  onChange: Change;
}): React.JSX.Element {
  const { draft, onChange } = props;
  const set = (index: number, patch: Partial<NewColumn>): void => {
    onChange({
      columns: draft.columns.map((column, at) =>
        at === index ? { ...column, ...patch } : column,
      ),
    });
  };
  return (
    <table className={styles.mapping} aria-label="New columns">
      <thead>
        <tr>
          <th>Import</th>
          <th>Column</th>
          <th>Type</th>
          <th>Inferred</th>
          <th>Primary key</th>
        </tr>
      </thead>
      <tbody>
        {draft.columns.map((column, index) => (
          <NewColumnRow
            key={props.sources[index]?.name ?? String(index)}
            column={column}
            source={props.sources[index]}
            onChange={(patch) => {
              set(index, patch);
            }}
          />
        ))}
      </tbody>
    </table>
  );
}

function NewColumnRow(props: {
  column: NewColumn;
  source: ImportColumnDto | undefined;
  onChange: (patch: Partial<NewColumn>) => void;
}): React.JSX.Element {
  const { column, source, onChange } = props;
  const label = source?.name ?? column.name;
  return (
    <tr>
      <td>
        <input
          type="checkbox"
          aria-label={`Import ${label}`}
          checked={column.include}
          onChange={(event) => {
            onChange({ include: event.target.checked });
          }}
        />
      </td>
      <td>
        <TextField
          aria-label={`Name of ${label}`}
          controlSize="compact"
          spellCheck={false}
          value={column.name}
          disabled={!column.include}
          onChange={(event) => {
            onChange({ name: event.target.value });
          }}
        />
      </td>
      <td>
        <TextField
          aria-label={`Type of ${label}`}
          controlSize="compact"
          spellCheck={false}
          value={column.type}
          disabled={!column.include}
          onChange={(event) => {
            onChange({ type: event.target.value });
          }}
        />
      </td>
      <td className={styles.typeCell}>
        {source ? categoryLabel(source.category) : ''}
        {source?.nullable ? ', nulls' : ''}
      </td>
      <td>
        <input
          type="checkbox"
          aria-label={`${label} is in the primary key`}
          checked={column.key}
          disabled={!column.include}
          onChange={(event) => {
            onChange({ key: event.target.checked });
          }}
        />
      </td>
    </tr>
  );
}
