// SPDX-License-Identifier: Apache-2.0
import { ToolConfirmation } from './ToolConfirmation';
import { Button, Icon } from '@openrows/ui';
import { ArtifactPreview, ArtifactRejected } from './ArtifactPreview';
import { RUNNING_OUTCOME } from './conversation';
import type { Turn } from './conversation';
type Step = Turn['traces'][number];
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
          {turn.traces.map((trace) => (
            <TraceStep key={trace.step} trace={trace} />
          ))}
          <Answer
            turn={turn}
            disabled={props.disabled}
            actions={props.actions}
          />
          {turn.confirmations.map((prompt) => (
            <ToolConfirmation
              key={prompt.id}
              prompt={prompt}
              disabled={turn.outcome !== RUNNING_OUTCOME}
            />
          ))}
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
          <p role="status" className={styles.outcome}>
            {turn.outcome}
          </p>
        </article>
      ))}
    </div>
  );
}
/** The streamed answer on a bubble, with its SQL proposals inside it. */
function Answer(props: {
  turn: Turn;
  disabled: boolean;
  actions: Actions;
}): React.JSX.Element | null {
  const { turn } = props;
  const proposals = [...new Set([...turn.proposals, ...sqlBlocks(turn.text)])];
  const text = proseOf(turn.text);
  if (!text && proposals.length === 0) return null;
  return (
    <div className={styles.bubble}>
      {text && <p className={styles.answer}>{text}</p>}
      {proposals.map((sql) => (
        <SqlProposal
          key={sql}
          sql={sql}
          disabled={props.disabled}
          actions={props.actions}
        />
      ))}
    </div>
  );
}
/** One tool call as a quiet line; its arguments and evidence open below it. */
function TraceStep({ trace }: { trace: Step }): React.JSX.Element {
  return (
    <details
      className={styles.tool}
      aria-label={t('toolStepStep', { step: String(trace.step) })}
    >
      <summary>
        <Icon name="check" />
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
  );
}
/** The answer without its complete fenced SQL, which shows as proposal cards. */
function proseOf(text: string): string {
  let shown = 0;
  return text
    .replace(/```sql\s*\n[\s\S]*?```/gi, (block) => (shown++ < 8 ? '' : block))
    .trim();
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
      <pre className={styles.code}>{sql}</pre>
      <p className={styles.caption}>{t('sqlProposalReview')}</p>
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
          variant="ghost"
          disabled={disabled}
          onClick={() => {
            actions.open(sql);
          }}
        >
          {t('openNewTab')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
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
