// SPDX-License-Identifier: Apache-2.0
import type { ValueParts } from './valueParts';
import styles from './Value.module.css';

const bound = (value: string | null, inclusive: boolean): string =>
  value === null
    ? 'unbounded'
    : `${value} (${inclusive ? 'inclusive' : 'exclusive'})`;

/** Array items, range bounds or address parts of a PostgreSQL value. */
export function StructuredValue({
  parts,
}: {
  parts: ValueParts;
}): React.JSX.Element {
  if (parts.kind === 'array')
    return (
      <ol className={styles.items} aria-label="Array items" start={1}>
        {parts.items.map((item, index) => (
          <li key={index} className={item === null ? styles.null : undefined}>
            {item ?? 'NULL'}
          </li>
        ))}
      </ol>
    );
  const rows: [string, string][] =
    parts.kind === 'network'
      ? [
          ['address', parts.address],
          ['prefix', parts.prefix ?? 'host'],
        ]
      : parts.empty
        ? [['range', 'empty']]
        : [
            ['lower', bound(parts.lower, parts.lowerInclusive)],
            ['upper', bound(parts.upper, parts.upperInclusive)],
          ];
  return (
    <dl className={styles.parts} aria-label="Value parts">
      {rows.map(([name, value]) => (
        <div key={name}>
          <dt>{name}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}
