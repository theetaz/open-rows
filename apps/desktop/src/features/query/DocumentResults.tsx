// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, Icon, IconButton, SegmentedControl } from '@openrows/ui';
import { JsonTree } from '../value/JsonTree';
import { useCellValue } from '../value/useCellValue';
import type { ResultMeta } from './run';
import styles from './DocumentResults.module.css';
import { t } from './lazyMessages';

type Mode = 'table' | 'tree' | 'json';

const MODES = [
  { value: 'table', label: t('table') },
  { value: 'tree', label: t('tree') },
  { value: 'json', label: t('json') },
] as const;

/** The whole-document EJSON column a MongoDB result carries (M3-01). */
function documentColumn(result: ResultMeta): number {
  return result.columns.findIndex(
    (column) => column.name === '__document' && column.sourceType === 'EJSON',
  );
}

/** Add document views only to a document result, keeping SQL results unchanged. */
export function DocumentView({
  result,
  children,
}: {
  result: ResultMeta;
  children: React.ReactNode;
}): React.JSX.Element {
  return documentColumn(result) >= 0 ? (
    <DocumentResults key={result.id} result={result}>
      {children}
    </DocumentResults>
  ) : (
    <>{children}</>
  );
}

/**
 * Table, tree and JSON views of a document result. The table is the canvas
 * grid; tree and JSON load one lossless document at a time, never every row.
 */
export function DocumentResults({
  result,
  children,
}: {
  result: ResultMeta;
  children: React.ReactNode;
}): React.JSX.Element {
  const [mode, setMode] = useState<Mode>('table');
  const [row, setRow] = useState(0);
  return (
    <div className={styles.view}>
      <div className={styles.bar} role="toolbar" aria-label={t('documentView')}>
        <SegmentedControl
          aria-label={t('showDocuments')}
          size="compact"
          options={MODES}
          value={mode}
          onValueChange={setMode}
        />
        {mode !== 'table' && (
          <DocumentPager row={row} rows={result.rows} onRow={setRow} />
        )}
      </div>
      {mode === 'table' ? (
        children
      ) : result.rows > 0 ? (
        <DocumentValue
          key={`${result.id}:${String(row)}`}
          address={{ resultId: result.id, row, column: documentColumn(result) }}
          mode={mode}
        />
      ) : (
        <p className={styles.note}>{t('noDocuments')}</p>
      )}
    </div>
  );
}

function DocumentPager(props: {
  row: number;
  rows: number;
  onRow: (row: number) => void;
}): React.JSX.Element {
  const { row, rows } = props;
  return (
    <span className={styles.pager}>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('previousDocument')}
        disabled={row === 0}
        onClick={() => {
          props.onRow(row - 1);
        }}
      >
        <Icon name="chevronLeft" />
      </IconButton>
      <span className={styles.position}>
        {t('documentValueRows', {
          value: (row + 1).toLocaleString(),
          rows: rows.toLocaleString(),
        })}
      </span>
      <IconButton
        size="sm"
        variant="ghost"
        aria-label={t('nextDocument')}
        disabled={row + 1 >= rows}
        onClick={() => {
          props.onRow(row + 1);
        }}
      >
        <Icon name="chevronRight" />
      </IconButton>
    </span>
  );
}

/** One document as a collapsible tree or indented canonical EJSON. */
function DocumentValue({
  address,
  mode,
}: {
  address: { resultId: string; row: number; column: number };
  mode: Exclude<Mode, 'table'>;
}): React.JSX.Element {
  const value = useCellValue(address);
  if (value.problem)
    return (
      <p className={styles.note} role="alert">
        {value.problem}
      </p>
    );
  if (!value.bytes)
    return (
      <div className={styles.note}>
        <Button size="sm" onClick={value.load} disabled={value.loading}>
          {value.loading ? t('loadingDocument') : t('loadDocument')}
        </Button>
      </div>
    );
  const text = new TextDecoder().decode(value.bytes);
  const parsed = parseDocument(text);
  return (
    <div className={styles.body}>
      {mode === 'tree' && parsed ? (
        <JsonTree value={parsed} />
      ) : (
        <pre className={styles.json}>
          {parsed ? JSON.stringify(parsed, null, 2) : text}
        </pre>
      )}
    </div>
  );
}

/**
 * Canonical EJSON wraps every number in a string (`{"$numberLong": "…"}`), so
 * parsing and re-indenting it loses no digits.
 */
function parseDocument(text: string): object | null {
  try {
    const value: unknown = JSON.parse(text);
    return value !== null && typeof value === 'object' ? value : null;
  } catch {
    return null;
  }
}
