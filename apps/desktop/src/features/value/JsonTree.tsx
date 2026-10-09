// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import styles from './Value.module.css';

/** Containers open on first view down to this depth; deeper ones start folded. */
const OPEN_DEPTH = 2;
/** Children listed per container at once; the rest is counted, not rendered. */
const MAX_CHILDREN = 200;

const isContainer = (value: unknown): value is object =>
  value !== null && typeof value === 'object';

/** The text of a scalar as JSON writes it: quoted strings, bare numbers and literals. */
const scalarText = (value: unknown): string => JSON.stringify(value);

/** A collapsible view of a parsed JSON object or array. */
export function JsonTree({ value }: { value: object }): React.JSX.Element {
  return (
    <ul className={styles.tree} aria-label="JSON tree">
      <JsonNode label="" value={value} depth={0} />
    </ul>
  );
}

function JsonNode(props: {
  label: string;
  value: unknown;
  depth: number;
}): React.JSX.Element {
  const { label, value, depth } = props;
  const [open, setOpen] = useState(depth < OPEN_DEPTH);
  if (!isContainer(value))
    return (
      <li className={styles.leaf}>
        {label && <span className={styles.key}>{label}</span>}
        <span className={styles.scalar}>{scalarText(value)}</span>
      </li>
    );
  const isArray = Array.isArray(value);
  const entries = Object.entries(value as Record<string, unknown>);
  const [opener, closer] = isArray ? ['[', ']'] : ['{', '}'];
  const folded = `${opener}${String(entries.length)} ${isArray ? 'items' : 'keys'}${closer}`;
  return (
    <li>
      <button
        type="button"
        className={styles.toggle}
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        {label && <span className={styles.key}>{label}</span>}
        <span className={styles.summary}>{open ? opener : folded}</span>
      </button>
      {open && (
        <ul>
          {entries.slice(0, MAX_CHILDREN).map(([key, child]) => (
            <JsonNode key={key} label={key} value={child} depth={depth + 1} />
          ))}
          {entries.length > MAX_CHILDREN && (
            <li className={styles.note}>
              {entries.length - MAX_CHILDREN} more not shown
            </li>
          )}
        </ul>
      )}
    </li>
  );
}
