// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { routineDefinition, tableDetail } from '@openrows/ipc';
import type { EngineDto, ObjectChangeDto, RoutineKindDto } from '@openrows/ipc';
import { ObjectSheet } from './ObjectSheet';
import { SqlField } from './SqlField';
import { routineTemplate, triggerTemplate } from './templates';
import { useDefinition } from './useDefinition';
import type { ChangeScope } from './useObjectChange';
import styles from '../table/structure/forms/Forms.module.css';

/** A trigger on a table, or a function or procedure; `name: null` is new. */
type DefinitionTarget =
  | { kind: 'trigger'; schema: string; table: string; name: string | null }
  | {
      kind: 'routine';
      schema: string;
      routine: RoutineKindDto;
      name: string | null;
      arguments: string | null;
    };

async function triggerDefinition(
  scope: ChangeScope,
  target: { schema: string; table: string; name: string },
): Promise<string> {
  const detail = await tableDetail({
    ...scope,
    schema: target.schema,
    table: target.table,
  });
  const trigger = detail.triggers.find((item) => item.name === target.name);
  if (!trigger) throw new Error(`The trigger ${target.name} was not found`);
  return `${trigger.definition};`;
}

async function readRoutine(
  scope: ChangeScope,
  target: Extract<DefinitionTarget, { kind: 'routine' }>,
): Promise<string> {
  const text = await routineDefinition({
    ...scope,
    schema: target.schema,
    name: target.name ?? '',
    routine: target.routine,
    arguments: target.arguments,
  });
  if (text === null)
    throw new Error(`The ${target.routine} ${target.name ?? ''} was not found`);
  return text;
}

/** The starting text: the stored definition, or a template for a new object. */
function source(
  scope: ChangeScope,
  engine: EngineDto,
  target: DefinitionTarget,
): { read: () => Promise<string> } | { value: string } {
  if (target.kind === 'trigger') {
    const { name } = target;
    return name === null
      ? { value: triggerTemplate(engine, target.schema, target.table) }
      : { read: () => triggerDefinition(scope, { ...target, name }) };
  }
  return target.name === null
    ? { value: routineTemplate(engine, target.schema, target.routine) }
    : { read: () => readRoutine(scope, target) };
}

/** The save request of the edited definition. */
function saveChange(
  target: DefinitionTarget,
  definition: string,
): ObjectChangeDto {
  if (target.kind === 'trigger')
    return {
      kind: 'saveTrigger',
      target: { schema: target.schema, name: target.table },
      replaces: target.name,
      definition,
    };
  return {
    kind: 'saveRoutine',
    target: { schema: target.schema, name: target.name ?? '' },
    routine: target.routine,
    replaces: target.name === null ? null : (target.arguments ?? ''),
    definition,
  };
}

function title(target: DefinitionTarget): string {
  const noun = target.kind === 'trigger' ? 'trigger' : target.routine;
  if (target.name === null)
    return target.kind === 'trigger'
      ? `New trigger on ${target.table}`
      : `New ${noun}`;
  return `Edit ${noun} ${target.name}`;
}

/**
 * The definition editor of triggers and routines: the full `CREATE`
 * statement in the SQL editor, checked by the driver (it must create
 * exactly this kind of object) and previewed with what replaces the old one.
 */
export function DefinitionEditor(props: {
  scope: ChangeScope;
  engine: EngineDto;
  target: DefinitionTarget;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
}): React.JSX.Element {
  const { scope, engine, target } = props;
  const loaded = useDefinition(
    JSON.stringify(target),
    source(scope, engine, target),
  );
  const [edited, setEdited] = useState<string | null>(null);
  const text = edited ?? (loaded.status === 'ready' ? loaded.value : '');
  return (
    <ObjectSheet
      title={title(target)}
      size="editor"
      scope={scope}
      change={
        loaded.status === 'ready' && text.trim()
          ? saveChange(target, text)
          : null
      }
      problem={loaded.status === 'error' ? loaded.message : null}
      action={target.name === null ? 'Create' : 'Save'}
      onClose={props.onClose}
      onDone={props.onDone}
    >
      {loaded.status === 'ready' ? (
        <SqlField
          value={text}
          label="Definition"
          engine={engine}
          onChange={setEdited}
        />
      ) : (
        <p className={styles.note} role="status">
          {loaded.status === 'loading' ? 'Reading the definition…' : ''}
        </p>
      )}
    </ObjectSheet>
  );
}
