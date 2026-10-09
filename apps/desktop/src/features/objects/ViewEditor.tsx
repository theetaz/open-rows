// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { TextField } from '@openrows/ui';
import { tableDetail } from '@openrows/ipc';
import type {
  EngineDto,
  ObjectChangeDto,
  ObjectFeaturesDto,
} from '@openrows/ipc';
import { ObjectSheet } from './ObjectSheet';
import { SqlField } from './SqlField';
import { useDefinition } from './useDefinition';
import type { Loaded } from './useDefinition';
import type { ChangeScope } from './useObjectChange';
import styles from '../table/structure/forms/Forms.module.css';

type Props = {
  scope: ChangeScope;
  engine: EngineDto;
  features: ObjectFeaturesDto;
  schema: string;
  /** The view to edit; `null` creates one. */
  name: string | null;
  /** A new view is materialized (existing views say so themselves). */
  materialized: boolean;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
};

/** A view's query and whether it is materialized. */
type ViewSource = { query: string; materialized: boolean };

/** The query of an existing view, read from its detail. */
async function readView(
  scope: ChangeScope,
  schema: string,
  name: string,
): Promise<ViewSource> {
  const detail = await tableDetail({ ...scope, schema, table: name });
  const { kind } = detail.info;
  if (
    kind !== 'view' &&
    kind !== 'materializedView' &&
    detail.viewQuery === null
  )
    throw new Error(`${name} is not a view`);
  return {
    query: detail.viewQuery ?? '',
    materialized: kind === 'materializedView',
  };
}

/** The starting query of a new view. */
const NEW_QUERY = 'SELECT\n  *\nFROM\n  ';

/** Name and (for a new view on PostgreSQL) the materialized switch. */
function ViewFields(props: {
  title: string;
  onTitle: (title: string) => void;
  /** An existing view keeps its name and kind. */
  existing: boolean;
  materialized: boolean;
  onMaterialized: ((materialized: boolean) => void) | null;
}): React.JSX.Element {
  const { onMaterialized } = props;
  return (
    <div className={styles.grid}>
      <TextField
        label="Name"
        value={props.title}
        readOnly={props.existing}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          props.onTitle(event.target.value);
        }}
      />
      {onMaterialized && (
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={props.materialized}
            onChange={(event) => {
              onMaterialized(event.target.checked);
            }}
          />
          Materialized
        </label>
      )}
    </div>
  );
}

/** The save request, or `null` while the name or query is missing. */
function saveView(
  view: { schema: string; name: string },
  query: string,
  options: { materialized: boolean; replace: boolean },
): ObjectChangeDto | null {
  if (!view.name.trim() || !query.trim()) return null;
  return {
    kind: 'saveView',
    target: { schema: view.schema, name: view.name.trim() },
    query,
    ...options,
  };
}

/** Where the editor's view comes from: a template, or the stored view. */
function viewSource(
  scope: ChangeScope,
  props: Pick<Props, 'schema' | 'name' | 'materialized'>,
): { value: ViewSource } | { read: () => Promise<ViewSource> } {
  const { schema, name } = props;
  if (name === null)
    return { value: { query: NEW_QUERY, materialized: props.materialized } };
  return { read: () => readView(scope, schema, name) };
}

/** The query in the SQL editor, or what is happening while it is read. */
function QueryArea(props: {
  loaded: Loaded<ViewSource>;
  text: string;
  engine: EngineDto;
  onChange: (query: string) => void;
}): React.JSX.Element {
  if (props.loaded.status === 'ready')
    return (
      <SqlField
        value={props.text}
        label="View query"
        engine={props.engine}
        onChange={props.onChange}
      />
    );
  return (
    <p className={styles.note} role="status">
      {props.loaded.status === 'loading' ? 'Reading the view…' : ''}
    </p>
  );
}

/** `New view`, `Edit materialized view daily`, … */
function sheetTitle(
  creating: boolean,
  materialized: boolean,
  name: string,
): string {
  const noun = materialized ? 'materialized view' : 'view';
  return creating ? `New ${noun}` : `Edit ${noun} ${name}`;
}

/**
 * Create a view or edit its query in the SQL editor; on PostgreSQL also
 * materialized views. The preview shows the `CREATE [OR REPLACE] VIEW`
 * (or the drop and create of a materialized view) that will run.
 */
export function ViewEditor(props: Props): React.JSX.Element {
  const { scope, schema, name } = props;
  const loaded = useDefinition<ViewSource>(
    `${schema}.${name ?? ''}`,
    viewSource(scope, props),
  );
  const [title, setTitle] = useState(name ?? '');
  const [chosen, setMaterialized] = useState(props.materialized);
  const [query, setQuery] = useState<string | null>(null);
  const source = loaded.status === 'ready' ? loaded.value : null;
  const text = query ?? source?.query ?? '';
  const creating = name === null;
  const materialized = creating ? chosen : Boolean(source?.materialized);
  const options = { materialized, replace: !creating };
  return (
    <ObjectSheet
      title={sheetTitle(creating, materialized, title)}
      size="editor"
      scope={scope}
      change={source && saveView({ schema, name: title }, text, options)}
      problem={loaded.status === 'error' ? loaded.message : null}
      action={creating ? 'Create' : 'Save'}
      onClose={props.onClose}
      onDone={props.onDone}
    >
      <ViewFields
        title={title}
        onTitle={setTitle}
        existing={!creating}
        materialized={materialized}
        onMaterialized={
          creating && props.features.materializedViews ? setMaterialized : null
        }
      />
      <QueryArea
        loaded={loaded}
        text={text}
        engine={props.engine}
        onChange={setQuery}
      />
    </ObjectSheet>
  );
}
