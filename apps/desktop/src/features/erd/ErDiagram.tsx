// SPDX-License-Identifier: Apache-2.0
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
  const state = useSchemaDiagram(props);
  return (
    <Sheet
      title={`ER Diagram — ${props.schema}`}
      size="appearance"
      onClose={props.onClose}
    >
      {state.status === 'ready' ? (
        state.diagram.tables.length ? (
          <DiagramView
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
