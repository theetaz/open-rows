// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import review from './Review.module.css';
import { t } from './messages/lazyData';

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
  /** A production banner above everything else. */
  warning?: ReactNode;
  /** The staged items, listed above the SQL. */
  items?: ReactNode;
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <Sheet title={props.title} onClose={props.onCancel}>
      {props.warning && (
        <p className={review.warning} role="note">
          {props.warning}
        </p>
      )}
      {props.items}
      <section className={review.section}>
        <h3 className={review.title}>{t('sqlCommit')}</h3>
        <pre className={review.sql} aria-label={t('sqlCommit')}>
          {props.sql}
        </pre>
      </section>
      <p className={review.summary}>{props.summary}</p>
      {props.children}
      {props.error && <p role="alert">{props.error}</p>}
      <SheetActions>
        <Button disabled={props.busy} onClick={props.onCancel}>
          {t('cancel')}
        </Button>
        <Button
          variant="primary"
          disabled={props.busy || props.blocked}
          onClick={props.onCommit}
        >
          {props.busy ? t('committing') : t('commitChanges')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
