// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { ArtifactPreview } from './ArtifactPreview';
import type { Turn } from './conversation';
import styles from './Assistant.module.css';
type Actions = {
  insert: (sql: string) => void;
  open: (sql: string) => void;
  explain: (sql: string) => void;
};
/** Plain text plus fenced SQL cards; provider output is never HTML or an execution command. */
export function ConversationView(props: {
  connection: string;
  turns: readonly Turn[];
  disabled: boolean;
  actions: Actions;
}): React.JSX.Element {
  return (
    <div
      className={styles.conversation}
      role="log"
      aria-label="Assistant conversation"
      aria-live="polite"
    >
      {props.turns.map((turn) => (
        <article key={turn.id}>
          <p className={styles.question}>{turn.question}</p>
          <p className={styles.answer}>{turn.text}</p>
          {turn.traces.map((trace) => (
            <details
              key={trace.step}
              aria-label={`Tool step ${String(trace.step)}`}
            >
              <summary>
                {trace.tool} · {trace.status} · {trace.durationMs} ms
              </summary>
              <p>{trace.arguments}</p>
              <p>
                {trace.bytes} bytes ·{' '}
                {trace.redacted ? 'redacted or truncated' : 'policy filtered'}
              </p>
            </details>
          ))}
          {[...new Set([...turn.proposals, ...sqlBlocks(turn.text)])].map(
            (sql) => (
              <SqlProposal
                key={sql}
                sql={sql}
                disabled={props.disabled}
                actions={props.actions}
              />
            ),
          )}
          {turn.artifacts.map((artifact, index) => (
            <ArtifactPreview
              key={index}
              {...artifact}
              connection={props.connection}
              disabled={props.disabled}
            />
          ))}
          <p role="status">{turn.outcome}</p>
        </article>
      ))}
    </div>
  );
}
function sqlBlocks(text: string): string[] {
  return [...text.matchAll(/```sql\s*\n([\s\S]*?)```/gi)]
    .map((match) => match[1]?.trim() ?? '')
    .filter(Boolean)
    .slice(0, 8);
}
function SqlProposal({
  sql,
  disabled,
  actions,
}: {
  sql: string;
  disabled: boolean;
  actions: Actions;
}): React.JSX.Element {
  return (
    <section aria-label="SQL proposal" className={styles.proposal}>
      <pre>{sql}</pre>
      <p>SQL proposal · review before running</p>
      <div className={styles.row}>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.insert(sql);
          }}
        >
          Insert into editor
        </Button>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.open(sql);
          }}
        >
          Open in new tab
        </Button>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.explain(sql);
          }}
        >
          Explain
        </Button>
      </div>
    </section>
  );
}
