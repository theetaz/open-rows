// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { BoardDto } from '@openrows/ipc';
import type { BoardRuns } from './useBoardRuns';
import styles from './Boards.module.css';

/** An imported board: every query shown, nothing runs until approved. */
export function Review(props: {
  board: BoardDto;
  onApprove: () => void;
}): React.JSX.Element {
  return (
    <section className={styles.review} aria-label="Review imported board">
      <strong>Imported board — paused.</strong>
      <span>
        Review every query below. Nothing runs until you approve this board.
      </span>
      {props.board.board.widgets.map((widget) => (
        <div key={widget.id}>
          <div>{widget.title}</div>
          <pre className={styles.sql}>{widget.sql}</pre>
        </div>
      ))}
      <div>
        <Button size="sm" variant="primary" onClick={props.onApprove}>
          Approve and enable
        </Button>
      </div>
    </section>
  );
}

export function RunNotes(props: {
  runs: BoardRuns;
  seconds: number | null;
}): React.JSX.Element {
  return (
    <>
      {props.runs.note && <p className={styles.note}>{props.runs.note}</p>}
      {props.runs.live && (
        <p className={styles.note}>
          Refreshing every {String(props.seconds)} s while this board is open.
        </p>
      )}
    </>
  );
}

/** An empty board offers exactly one action. */
export function EmptyBoard(props: { onAdd: () => void }): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>This board has no widgets.</p>
      <Button size="sm" variant="primary" onClick={props.onAdd}>
        Add widget
      </Button>
    </div>
  );
}
