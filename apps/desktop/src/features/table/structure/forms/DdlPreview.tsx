// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';
import type { PreviewState } from './usePreview';
import styles from './Forms.module.css';

/** `a;\nb;`: statements as they run, each ending in `;`. */
const sqlText = (statements: readonly string[]): string =>
  statements.map((statement) => `${statement};`).join('\n');

/**
 * The live DDL preview: exactly the statements that will run, their
 * warnings (locks, rewrites, data loss), or why nothing can be generated.
 */
export function DdlPreview({
  state,
  idle = 'Nothing to change yet.',
  children,
}: {
  state: PreviewState;
  /** Shown while there is nothing to preview. */
  idle?: string;
  /** Extra content after the warnings (e.g. a confirmation field). */
  children?: ReactNode;
}): React.JSX.Element {
  return (
    <section className={styles.preview} aria-label="SQL preview">
      <h3>SQL preview</h3>
      {state.status === 'idle' && <p className={styles.note}>{idle}</p>}
      {state.status === 'loading' && (
        <p className={styles.note} role="status">
          Generating…
        </p>
      )}
      {state.status === 'error' && (
        <p className={styles.problem} role="alert">
          {state.message}
        </p>
      )}
      {state.status === 'ready' && (
        <>
          <pre className={styles.sql} aria-label="Statements to run">
            {sqlText(state.statements)}
          </pre>
          {state.warnings.length > 0 && (
            <ul className={styles.warnings} aria-label="Warnings">
              {state.warnings.map((warning) => (
                <li key={warning}>{warning}</li>
              ))}
            </ul>
          )}
          {children}
        </>
      )}
    </section>
  );
}
