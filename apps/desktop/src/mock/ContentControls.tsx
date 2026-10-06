// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, IconButton, ToolbarGroup } from '@openrows/ui';
import styles from './Content.module.css';

/** Floating summary of the static change set. */
export function CommitCapsule({
  onDiscard,
  onReview,
}: {
  onDiscard: () => void;
  onReview: () => void;
}): React.JSX.Element {
  return (
    <ToolbarGroup aria-label="Pending changes" className={styles.commit}>
      <span className={styles.changeDot} aria-hidden="true">
        ●
      </span>
      <strong>4 changes</strong>
      <Button size="sm" onClick={onDiscard}>
        Discard
      </Button>
      <Button size="sm" variant="primary" onClick={onReview}>
        Review & Commit
      </Button>
    </ToolbarGroup>
  );
}
/** Mock pagination retains the reference timing and row estimate. */
export function ContentFooter(): React.JSX.Element {
  const [page, setPage] = useState(0);
  return (
    <footer className={styles.footer}>
      <ToolbarGroup aria-label="Pagination">
        <IconButton
          aria-label="Previous page"
          size="sm"
          variant="ghost"
          onClick={() => {
            setPage(Math.max(0, page - 1));
          }}
        >
          ‹
        </IconButton>
        <span>{page === 0 ? '1–200' : '201–400'} of ~2.4M rows</span>
        <IconButton
          aria-label="Next page"
          size="sm"
          variant="ghost"
          onClick={() => {
            setPage(page === 0 ? 1 : 0);
          }}
        >
          ›
        </IconButton>
      </ToolbarGroup>
      <span className={styles.timing}>Query 38 ms · auto-commit</span>
    </footer>
  );
}
