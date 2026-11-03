// SPDX-License-Identifier: Apache-2.0
import { ToolConfirmation } from './ToolConfirmation';
import { Button } from '@openrows/ui';
import { ArtifactPreview, ArtifactRejected } from './ArtifactPreview';
import { RUNNING_OUTCOME } from './conversation';
import type { Turn } from './conversation';
import styles from './Assistant.module.css';
import { t } from './lazyMessages';
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
      aria-label={t('assistantConversation')}
      aria-live="polite"
    >
      {props.turns.map((turn) => (
        <article key={turn.id}>
          <p className={styles.question}>{turn.question}</p>
          <p className={styles.answer}>{turn.text}</p>
          {turn.confirmations.map((prompt) => (
            <ToolConfirmation
              key={prompt.id}
              prompt={prompt}
              disabled={turn.outcome !== RUNNING_OUTCOME}
            />
          ))}
          {turn.traces.map((trace) => (
            <details
              key={trace.step}
              aria-label={t('toolStepStep', { step: String(trace.step) })}
            >
              <summary>
                {t('labelStatusDurationms', {
                  label: trace.label ?? trace.tool,
                  status: trace.status,
                  durationMs: trace.durationMs,
                })}
              </summary>
              <p>{trace.arguments}</p>
              {trace.result && (
                <pre aria-label={t('mcpToolResult')}>{trace.result}</pre>
              )}
              <p>
                {trace.redacted
                  ? t('bytesBytesRedacted', { bytes: trace.bytes })
                  : t('bytesBytesPolicy', { bytes: trace.bytes })}
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
          {turn.rejections.map((rejection, index) => (
            <ArtifactRejected key={index} {...rejection} />
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
    <section aria-label={t('sqlProposal')} className={styles.proposal}>
      <pre>{sql}</pre>
      <p>{t('sqlProposalReview')}</p>
      <div className={styles.row}>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.insert(sql);
          }}
        >
          {t('insertIntoEditor')}
        </Button>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.open(sql);
          }}
        >
          {t('openNewTab')}
        </Button>
        <Button
          size="sm"
          disabled={disabled}
          onClick={() => {
            actions.explain(sql);
          }}
        >
          {t('explain')}
        </Button>
      </div>
    </section>
  );
}
