// SPDX-License-Identifier: Apache-2.0
import styles from './Content.module.css';

const keywords =
  /^(SELECT|AS|FROM|WHERE|AND|GROUP|BY|ORDER|DESC|CREATE|TABLE|PRIMARY|KEY|NOT|NULL|DEFAULT|INDEX|ON|BEGIN|UPDATE|SET|INSERT|INTO|VALUES|DELETE|COMMIT|interval)$/i;
/** Small mock-only SQL formatter; no parser, execution or editor dependency. */
export function SqlCode({ sql }: { sql: string }): React.JSX.Element {
  return (
    <pre className={styles.code}>
      <code>
        {sql.split(/('[^']*'|\b[A-Za-z_]+\b)/g).map((part, index) => (
          <span
            key={index}
            data-token={
              part.startsWith("'")
                ? 'string'
                : keywords.test(part)
                  ? 'keyword'
                  : ['count', 'sum', 'now'].includes(part)
                    ? 'function'
                    : undefined
            }
          >
            {part}
          </span>
        ))}
      </code>
    </pre>
  );
}
