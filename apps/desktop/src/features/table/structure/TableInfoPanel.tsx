// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { TableInfoDto } from '@openrows/ipc';
import { TextArea } from '@openrows/ui';
import { formatBytes, formatRows } from './format';
import styles from './Structure.module.css';
import { t } from '../messages/initial';

/** What the inspector shows for a table tab in Structure or Index mode. */
export type SelectedTable = {
  name: string;
  info: TableInfoDto;
  /** Comment with any staged change applied. */
  comment: string | null;
  /** Stage `COMMENT ON TABLE`; absent when the table cannot be changed. */
  onComment?: ((comment: string | null) => void) | undefined;
};

/** Sizes, estimated rows and the (editable) comment of a table. */
export function TableInfoPanel({
  table,
}: {
  table: SelectedTable;
}): React.JSX.Element {
  const { info } = table;
  return (
    <dl
      className={styles.info}
      aria-label={t('nameTableInfo', { name: table.name })}
    >
      <dt>{t('totalSize')}</dt>
      <dd>{formatBytes(info.totalBytes)}</dd>
      <dt>{t('dataSize')}</dt>
      <dd>{formatBytes(info.dataBytes)}</dd>
      <dt>{t('indexSize')}</dt>
      <dd>{formatBytes(info.indexBytes)}</dd>
      <dt>{t('estimatedRows')}</dt>
      <dd>{formatRows(info.rowEstimate)}</dd>
      <dt>{t('comment')}</dt>
      <dd className={styles.comment}>
        <CommentField
          key={`${table.name}:${table.comment ?? ''}`}
          table={table}
        />
      </dd>
    </dl>
  );
}

function CommentField({ table }: { table: SelectedTable }): React.JSX.Element {
  const [draft, setDraft] = useState(table.comment ?? '');
  return (
    <TextArea
      controlSize="box"
      aria-label={t('comment')}
      className={styles.value}
      value={draft}
      placeholder={t('null')}
      readOnly={!table.onComment}
      spellCheck={false}
      onChange={(event) => {
        setDraft(event.target.value);
      }}
      onBlur={() => {
        if (draft !== (table.comment ?? '')) table.onComment?.(draft || null);
      }}
    />
  );
}
