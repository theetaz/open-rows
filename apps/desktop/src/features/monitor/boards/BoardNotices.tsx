// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import type { BoardDto } from '@openrows/ipc';
import type { BoardRuns } from './useBoardRuns';
import styles from './Boards.module.css';
import { t } from '../messages/lazyBoards';

/** An imported board: every query shown, nothing runs until approved. */
export function Review(props: {
  board: BoardDto;
  onApprove: () => void;
}): React.JSX.Element {
  return (
    <section className={styles.review} aria-label={t('reviewImportedBoard')}>
      <strong>{t('importedBoardPaused')}</strong>
      <span>{t('reviewEveryQuery')}</span>
      {props.board.board.widgets.map((widget) => (
        <div key={widget.id}>
          <div>{widget.title}</div>
          <pre className={styles.sql}>{widget.sql}</pre>
        </div>
      ))}
      <div>
        <Button size="sm" variant="primary" onClick={props.onApprove}>
          {t('approveEnable')}
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
          {t('refreshingEverySeconds', { seconds: String(props.seconds) })}
        </p>
      )}
    </>
  );
}

/** An empty board offers exactly one action. */
export function EmptyBoard(props: { onAdd: () => void }): React.JSX.Element {
  return (
    <div className={styles.empty}>
      <p>{t('boardNoWidgets')}</p>
      <Button size="sm" variant="primary" onClick={props.onAdd}>
        {t('addWidget')}
      </Button>
    </div>
  );
}
