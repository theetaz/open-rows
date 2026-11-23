// SPDX-License-Identifier: Apache-2.0
import type { Draft } from '../form';
import { engineName, isFileEngine } from '../engines';
import { POLICY_LABELS } from '../../security/policies';
import { derivedName } from './flowDraft';
import type { StepId } from './flowDraft';
import { FlowIcon } from './FlowIcon';
import type { FlowSteps } from './useFlowSteps';
import styles from './Flow.module.css';
import { t } from './messages';

const NAMES: Record<StepId, string> = {
  engine: t('engineStep'),
  details: t('detailsStep'),
  security: t('securityStep'),
  options: t('optionsStep'),
  test: t('testStep'),
};
const SUBS: Record<StepId, string> = {
  engine: t('engineSub'),
  details: t('detailsSub'),
  security: t('securitySub'),
  options: t('optionsSub'),
  test: t('testSub'),
};

/** A step's name; file engines call Details "File". */
function stepName(step: StepId, draft: Draft): string {
  return step === 'details' && isFileEngine(draft.engine)
    ? t('fileStep')
    : NAMES[step];
}

/** What a finished step chose, or what the step is for. */
function stepSub(step: StepId, draft: Draft, done: boolean): string {
  if (step === 'engine' && done) return engineName(draft.engine);
  if (step === 'details' && done) return draft.name || derivedName(draft);
  if (step === 'details' && isFileEngine(draft.engine)) return t('fileSub');
  if (step === 'options' && done)
    return `${draft.envKind} · ${POLICY_LABELS[draft.safety]}`;
  return SUBS[step];
}

/** The steps column: number or tick, name and summary; later steps locked. */
export function FlowRail(props: {
  steps: FlowSteps;
  draft: Draft;
  tested: boolean;
  title: string;
}): React.JSX.Element {
  const { steps, draft } = props;
  const index = steps.order.indexOf(steps.current);
  return (
    <nav className={styles.rail} aria-label={t('steps')}>
      <p className={styles.railTitle}>{props.title}</p>
      {steps.order.map((step, position) => {
        const done = position < index || (step === 'test' && props.tested);
        return (
          <button
            key={step}
            type="button"
            className={styles.step}
            aria-current={step === steps.current ? 'step' : undefined}
            data-done={done}
            disabled={position > steps.reached}
            onClick={() => {
              steps.go(step);
            }}
          >
            <span className={styles.badge}>
              {done && step !== steps.current ? (
                <FlowIcon name="check" />
              ) : (
                position + 1
              )}
            </span>
            <span className={styles.lines}>
              <span>{stepName(step, draft)}</span>
              <span className={styles.sub}>{stepSub(step, draft, done)}</span>
            </span>
          </button>
        );
      })}
    </nav>
  );
}
