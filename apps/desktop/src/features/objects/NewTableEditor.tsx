// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import type {
  EngineDto,
  NewCheckDto,
  ObjectChangeDto,
  ObjectFeaturesDto,
} from '@openrows/ipc';
import {
  CheckFields,
  checkProblem,
} from '../table/structure/forms/CheckFields';
import {
  emptyForeignKey,
  ForeignKeyFields,
  foreignKeyProblem,
} from '../table/structure/forms/ForeignKeyFields';
import type { ForeignKeyDraft } from '../table/structure/forms/ForeignKeyFields';
import {
  emptyIndex,
  IndexFields,
  indexProblem,
} from '../table/structure/forms/IndexFields';
import type { IndexDraft } from '../table/structure/forms/IndexFields';
import { ColumnsSection } from './ColumnsSection';
import { ListSection } from './ListSection';
import type { ItemForm } from './ListSection';
import { createTableChange, emptyTable, tableProblem } from './newTable';
import type { TableDraft } from './newTable';
import { ObjectSheet } from './ObjectSheet';
import type { ChangeScope } from './useObjectChange';
import styles from '../table/structure/forms/Forms.module.css';

type Props = {
  scope: ChangeScope;
  engine: EngineDto;
  features: ObjectFeaturesDto;
  schema: string;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
};

const removeAt = <T,>(items: readonly T[], index: number): T[] =>
  items.filter((_, at) => at !== index);

type SectionProps = Props & {
  draft: TableDraft;
  onChange: (draft: TableDraft) => void;
};

/** Named columns of the draft, for keys and indexes. */
const named = (draft: TableDraft): string[] =>
  draft.columns.map((column) => column.name.trim()).filter(Boolean);

function IndexesSection({
  draft,
  onChange,
  engine,
  features,
}: SectionProps): React.JSX.Element {
  return (
    <ListSection<IndexDraft>
      title="Indexes"
      items={draft.indexes}
      describe={(index) =>
        `${index.unique ? 'UNIQUE ' : ''}${index.name?.trim() || '(automatic)'} (${index.columns.join(', ')})`
      }
      onRemove={(index) => {
        onChange({ ...draft, indexes: removeAt(draft.indexes, index) });
      }}
      form={{
        empty: emptyIndex,
        problem: indexProblem,
        render: (index, set) => (
          <IndexFields
            draft={index}
            onChange={set}
            columns={named(draft)}
            engine={engine}
            features={features}
          />
        ),
        onAdd: (index) => {
          onChange({ ...draft, indexes: [...draft.indexes, index] });
        },
      }}
    />
  );
}

function ForeignKeysSection(props: SectionProps): React.JSX.Element {
  const { draft, onChange } = props;
  const form: ItemForm<ForeignKeyDraft> = {
    empty: emptyForeignKey(props.schema),
    problem: foreignKeyProblem,
    render: (key, set) => (
      <ForeignKeyFields
        draft={key}
        onChange={set}
        columns={named(draft)}
        scope={props.scope}
      />
    ),
    onAdd: (key) => {
      onChange({ ...draft, foreignKeys: [...draft.foreignKeys, key] });
    },
  };
  return (
    <ListSection<ForeignKeyDraft>
      title="Foreign keys"
      items={draft.foreignKeys}
      describe={(key) =>
        `(${key.columns.join(', ')}) → ${key.target.table} (${key.target.columns.join(', ')})`
      }
      onRemove={(index) => {
        onChange({ ...draft, foreignKeys: removeAt(draft.foreignKeys, index) });
      }}
      {...(props.features.foreignKeys ? { form } : {})}
    />
  );
}

function ChecksSection(props: SectionProps): React.JSX.Element {
  const { draft, onChange } = props;
  const form: ItemForm<NewCheckDto> = {
    empty: { name: null, expression: '' },
    problem: checkProblem,
    render: (check, set) => <CheckFields check={check} onChange={set} />,
    onAdd: (check) => {
      onChange({ ...draft, checks: [...draft.checks, check] });
    },
  };
  return (
    <ListSection<NewCheckDto>
      title="Checks"
      items={draft.checks}
      describe={(check) =>
        `${check.name?.trim() ?? ''} CHECK (${check.expression})`
      }
      onRemove={(index) => {
        onChange({ ...draft, checks: removeAt(draft.checks, index) });
      }}
      {...(props.features.checks ? { form } : {})}
    />
  );
}

/**
 * The new-table editor (TablePlus "New table"): name, columns with the
 * Structure view's column form, primary key, indexes, foreign keys and
 * checks, with the `CREATE TABLE` that will run previewed live.
 */
export function NewTableEditor(props: Props): React.JSX.Element {
  const [draft, setDraft] = useState(() =>
    emptyTable(props.engine, props.features),
  );
  const problem = tableProblem(draft);
  return (
    <ObjectSheet
      title={`New table in ${props.schema}`}
      size="editor"
      scope={props.scope}
      change={problem ? null : createTableChange(draft, props.schema)}
      problem={problem}
      action="Create table"
      onClose={props.onClose}
      onDone={props.onDone}
    >
      <div className={styles.grid}>
        <TextField
          label="Table name"
          value={draft.name}
          autoComplete="off"
          spellCheck={false}
          onChange={(event) => {
            setDraft({ ...draft, name: event.target.value });
          }}
        />
        {props.features.comments && (
          <TextField
            label="Comment"
            value={draft.comment}
            onChange={(event) => {
              setDraft({ ...draft, comment: event.target.value });
            }}
          />
        )}
      </div>
      <ColumnsSection
        draft={draft}
        onChange={setDraft}
        engine={props.engine}
        features={props.features}
      />
      <IndexesSection {...props} draft={draft} onChange={setDraft} />
      <ForeignKeysSection {...props} draft={draft} onChange={setDraft} />
      <ChecksSection {...props} draft={draft} onChange={setDraft} />
    </ObjectSheet>
  );
}
