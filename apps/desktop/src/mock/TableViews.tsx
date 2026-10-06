// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import {
  structures,
  indexes,
  triggers,
  ddl,
  assistantSql,
} from './data/content';
import { SqlCode } from './SqlCode';
import styles from './Content.module.css';

/** Hard-coded schema views; each makes the table switcher inspectable. */
export function TableView({ view }: { view: string }): React.JSX.Element {
  if (view === 'DDL')
    return (
      <div className={styles.view}>
        <SqlCode sql={ddl} />
      </div>
    );
  if (view === 'Triggers')
    return (
      <div className={styles.view}>
        <h2>Triggers</h2>
        {triggers.map(([name, timing, functionName]) => (
          <article className={styles.trigger} key={name}>
            <strong>{name}</strong>
            <p>{timing}</p>
            <code>{functionName}</code>
          </article>
        ))}
      </div>
    );
  const headings =
    view === 'Structure'
      ? ['Column', 'Type', 'Constraint', 'Default']
      : ['Index', 'Method', 'Columns', 'Uniqueness'];
  return (
    <div className={styles.view}>
      <table className={styles.schemaTable}>
        <thead>
          <tr>
            {headings.map((heading) => (
              <th key={heading}>{heading}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {(view === 'Structure' ? structures : indexes).map((fields, row) => (
            <tr key={row}>
              {fields.map((value, column) => (
                <td key={column}>{value}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
/** Lightweight editable SQL mock with visual action feedback only. */
export function QueryView({
  sql = assistantSql,
  onChange,
}: {
  sql?: string | undefined;
  onChange: (sql: string) => void;
}): React.JSX.Element {
  return (
    <div className={styles.view}>
      <div className={styles.queryActions}>
        <Button size="sm" variant="primary">
          Run read-only
        </Button>
        <Button size="sm">Explain</Button>
      </div>
      <textarea
        className={styles.editor}
        aria-label="SQL editor"
        value={sql}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        spellCheck={false}
      />
      <p className={styles.secondary}>Query preview · ready</p>
    </div>
  );
}
