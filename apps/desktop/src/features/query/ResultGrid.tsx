// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { EditToolbar, rowEditBar } from '../table/EditToolbar';
import { RowEditSheet } from '../table/RowEditSheet';
import { DiscardSheet } from '../table/DiscardSheet';
import { ResultGridBody } from './ResultGridBody';
import { useResultGrid } from './useResultGrid';
import type { ResultGridProps } from './useResultGrid';
import styles from './Query.module.css';
import { t } from './lazyMessages';
/** Query results use the same canvas, inline editor and staged row tools as table tabs. */
export function ResultGrid(props: ResultGridProps): React.JSX.Element {
  const model = useResultGrid(props);
  const { editing, reason } = model.edits;
  const [discard, setDiscard] = useState(false);
  return (
    <div className={styles.resultView}>
      <ResultGridBody
        model={model}
        result={props.result}
        onExport={props.onExport}
      />
      <div
        className={styles.resultStatus}
        role="status"
        aria-label={t('resultStatus')}
      >
        {[
          reason ? t('readOnlyReason', { reason }) : t('doubleClickCellPress'),
          model.view.status,
          model.tools.aggregate,
        ]
          .filter(Boolean)
          .join(' · ')}
      </div>
      {editing.error && <p role="alert">{editing.error}</p>}
      <EditToolbar
        model={rowEditBar(editing, model.edits.ready, () => {
          setDiscard(true);
        })}
      />
      <RowEditSheet editing={editing} />
      {model.context.menu}
      {discard && (
        <DiscardSheet
          kind="discard"
          count={editing.history.current.length}
          onClose={() => {
            setDiscard(false);
          }}
          onKeep={() => {
            setDiscard(false);
          }}
          onDiscard={() => {
            editing.discard();
            setDiscard(false);
          }}
        />
      )}
    </div>
  );
}
