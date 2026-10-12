// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { TablePicker } from './TablePicker';
import { Sheet } from '@openrows/ui/sheet';
import { DiagramView } from './DiagramView';
import { useSchemaDiagram } from './useSchemaDiagram';
import styles from './Erd.module.css';

export type ErDiagramProps = {
  profileId: string;
  database: string;
  schema: string;
  /** Table to centre on first (opened from its context menu). */
  focus?: string | undefined;
  tables?: string[] | undefined;
  /** Open a table of the schema (the sheet closes first). */
  onOpen: (table: string) => void;
  onClose: () => void;
};

/**
 * Read-only ER diagram of one schema (FR-SCHEMA-06): tables with their
 * columns and the foreign keys between them, auto-laid out, with pan, zoom,
 * find and SVG/PNG export.
 */
export function ErDiagram(props: ErDiagramProps): React.JSX.Element {
  const [tables, setTables] = useState(props.tables);
  const [picking, setPicking] = useState(false);
  const target = {
    profileId: props.profileId,
    database: props.database,
    schema: props.schema,
    ...(tables ? { tables } : {}),
  };
  const state = useSchemaDiagram(target);
  return (
    <Sheet
      title={`ER Diagram — ${props.schema}`}
      size="appearance"
      onClose={props.onClose}
    >
      <Button
        size="sm"
        onClick={() => {
          setPicking(true);
        }}
      >
        Choose tables…
      </Button>
      {picking && (
        <TablePicker
          target={target}
          onClose={() => {
            setPicking(false);
          }}
          onSelect={(names) => {
            setTables(names);
            setPicking(false);
          }}
        />
      )}
      {state.status === 'ready' ? (
        state.diagram.tables.length ? (
          <DiagramView
            key={JSON.stringify(tables)}
            schema={props.schema}
            diagram={state.diagram}
            layout={state.layout}
            focus={props.focus}
            onOpen={(table) => {
              props.onClose();
              props.onOpen(table);
            }}
            onDone={props.onClose}
          />
        ) : (
          <p className={styles.note}>{`${props.schema} has no tables`}</p>
        )
      ) : state.status === 'error' ? (
        <p className={styles.error} role="alert">
          {state.message}
        </p>
      ) : (
        <p className={styles.note} role="status">
          Loading diagram…
        </p>
      )}
    </Sheet>
  );
}
