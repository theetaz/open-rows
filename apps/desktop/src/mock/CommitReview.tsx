// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl, Sheet } from '@openrows/ui';
import { changes, changeSql, reviewSegments } from './data/content';
import { SqlCode } from './SqlCode';
import styles from './Content.module.css';

/** UI-only review sheet: inspect changes or SQL without executing a write. */
export function CommitReview({
  onClose,
  onDiscard,
}: {
  onClose: () => void;
  onDiscard: () => void;
}): React.JSX.Element {
  const [view, setView] = useState('Changes');
  return (
    <Sheet title="Review & Commit" onClose={onClose}>
      <p className={styles.secondary}>4 changes · public.orders</p>
      <SegmentedControl
        aria-label="Review view"
        options={reviewSegments}
        value={view}
        onValueChange={setView}
      />
      {view === 'SQL' ? (
        <SqlCode sql={changeSql} />
      ) : (
        <ul className={styles.changes}>
          {changes.map((change) => (
            <li key={change}>{change}</li>
          ))}
        </ul>
      )}
      <div className={styles.reviewActions}>
        <Button size="sm" onClick={onDiscard}>
          Discard
        </Button>
        <Button size="sm" onClick={onClose}>
          Cancel
        </Button>
        <Button size="sm" variant="primary" onClick={onClose}>
          Commit 4 changes
        </Button>
      </div>
    </Sheet>
  );
}
