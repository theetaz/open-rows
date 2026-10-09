// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { BinaryValue } from './BinaryValue';
import { TextValue } from './TextValue';
import { byteLenOf, useCellValue } from './useCellValue';
import type { CellAddress, CellValueState } from './useCellValue';
import { formatBytes } from './valueFormat';
import styles from './Value.module.css';

type Props = {
  address: CellAddress;
  /** The column name, for the accessible name. */
  name: string;
  /** Database type name, e.g. `jsonb`. */
  type: string | null;
  /** Display text, shown for values without bytes (numbers, dates). */
  text: string | null;
};

/**
 * The full value of one result cell: JSON tree or text, hex dump or image
 * preview. Values above the automatic-load size wait for "Load full value".
 * Callers key this by cell, so each cell starts from a fresh load.
 */
export function ValueInspector(props: Props): React.JSX.Element {
  const value = useCellValue(props.address);
  const kind = value.info?.kind;
  const bytesKnown = kind === 'text' || kind === 'binary';
  return (
    <section className={styles.inspector} aria-label={`Value of ${props.name}`}>
      <div className={styles.meta}>
        {props.type && <small>{props.type}</small>}
        {bytesKnown && value.info && (
          <small>{formatBytes(byteLenOf(value.info))}</small>
        )}
      </div>
      <ValueBody value={value} text={props.text} />
    </section>
  );
}

function ValueBody(props: {
  value: CellValueState;
  text: string | null;
}): React.JSX.Element {
  const { value, text } = props;
  if (value.problem) return <p role="alert">{value.problem}</p>;
  const { info } = value;
  if (!info) return <p role="status">Loading…</p>;
  if (info.kind === 'null') return <p className={styles.null}>NULL</p>;
  if (info.kind === 'other')
    return <pre className={styles.text}>{text ?? ''}</pre>;
  if (value.bytes)
    return info.kind === 'binary' ? (
      <BinaryValue bytes={value.bytes} />
    ) : (
      <TextValue bytes={value.bytes} />
    );
  if (value.loading) return <p role="status">Loading full value…</p>;
  return (
    <div className={styles.large}>
      <p>{formatBytes(byteLenOf(info))}: larger than the automatic preview.</p>
      <Button size="sm" onClick={value.load}>
        Load full value
      </Button>
    </div>
  );
}
