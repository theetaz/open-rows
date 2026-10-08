// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import styles from './Table.module.css';

/** The transactional commit preview shared by row and structure edits. */
export function ReviewSheet(props: {
  title: string;
  summary: ReactNode;
  /** Exactly the SQL that will run. */
  sql: string;
  error: string | null;
  busy: boolean;
  /** Commit is not possible (uncertain outcome, missing confirmation). */
  blocked: boolean;
  onCancel: () => void;
  onCommit: () => void;
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <Sheet title={props.title} onClose={props.onCancel}>
      <p>{props.summary}</p>
      <pre className={styles.preview} aria-label="SQL to commit">
        {props.sql}
      </pre>
      {props.children}
      {props.error && <p role="alert">{props.error}</p>}
      <SheetActions>
        <Button disabled={props.busy} onClick={props.onCancel}>
          Cancel
        </Button>
        <Button
          variant="primary"
          disabled={props.busy || props.blocked}
          onClick={props.onCommit}
        >
          {props.busy ? 'Committing…' : 'Commit changes'}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
