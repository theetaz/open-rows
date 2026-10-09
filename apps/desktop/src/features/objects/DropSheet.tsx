// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { EngineDto, ObjectChangeDto } from '@openrows/ipc';
import { cascades } from '../table/structure/forms/dialect';
import { ObjectSheet } from './ObjectSheet';
import { objectNoun } from './requests';
import type { DropTarget } from './requests';
import styles from '../table/structure/forms/Forms.module.css';

/** The drop statement's request; `cascade` where the object takes it. */
export function dropChange(
  target: DropTarget,
  cascade: boolean,
): ObjectChangeDto {
  switch (target.object) {
    case 'table':
      return {
        kind: 'dropTable',
        target: { schema: target.schema, name: target.name },
        cascade,
      };
    case 'view':
    case 'materializedView':
      return {
        kind: 'dropView',
        target: { schema: target.schema, name: target.name },
        materialized: target.object === 'materializedView',
        cascade,
      };
    case 'trigger':
      return {
        kind: 'dropTrigger',
        target: { schema: target.schema, name: target.table },
        name: target.name,
      };
    case 'function':
    case 'procedure':
      return {
        kind: 'dropRoutine',
        target: { schema: target.schema, name: target.name },
        routine: target.object,
        arguments: target.arguments,
      };
    case 'database':
      return { kind: 'dropDatabase', name: target.name };
    case 'schema':
      return { kind: 'dropSchema', name: target.name, cascade };
  }
}

/** Objects whose drop takes `CASCADE`. */
const cascading = new Set<DropTarget['object']>([
  'table',
  'view',
  'materializedView',
  'schema',
]);

/**
 * Drop an object: the exact statement, its warnings and (on PostgreSQL)
 * CASCADE. A database open in the app must be closed first.
 */
export function DropSheet(props: {
  target: DropTarget;
  engine: EngineDto;
  profileId: string;
  /** Databases open in the app. */
  opened: readonly string[];
  onCloseDatabase: (name: string) => void;
  onClose: () => void;
  onDone: (change: ObjectChangeDto) => void;
}): React.JSX.Element {
  const { target } = props;
  const [cascade, setCascade] = useState(false);
  const noun = objectNoun[target.object];
  const open =
    target.object === 'database' && props.opened.includes(target.name);
  return (
    <ObjectSheet
      title={`Drop ${noun} ${target.name}`}
      scope={{ profileId: props.profileId, database: target.database }}
      change={open ? null : dropChange(target, cascade)}
      problem={
        open ? `${target.name} is open in OpenRows; close it to drop it.` : null
      }
      action="Drop"
      destructive
      onClose={props.onClose}
      onDone={props.onDone}
    >
      <p>
        The {noun} {target.name} is removed permanently.
      </p>
      {open && (
        <Button
          onClick={() => {
            props.onCloseDatabase(target.name);
          }}
        >
          Close {target.name}
        </Button>
      )}
      {cascades(props.engine) && cascading.has(target.object) && (
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={cascade}
            onChange={(event) => {
              setCascade(event.target.checked);
            }}
          />
          Also drop the objects that depend on it (CASCADE)
        </label>
      )}
    </ObjectSheet>
  );
}
