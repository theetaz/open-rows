// SPDX-License-Identifier: Apache-2.0
import { Button, Kbd } from '@openrows/ui';
import type { FlowSteps } from './useFlowSteps';
import styles from './Flow.module.css';
import { t } from './messages';

/**
 * Cancel and the step count leading; Back, then Continue ↩ — or, on the
 * last step, Save (and Save and Connect when the host offers it). An edit
 * can save from any step.
 */
export function FlowFooter(props: {
  steps: FlowSteps;
  editing: boolean;
  /** An engine is chosen, so the flow can continue. */
  ready: boolean;
  saving: boolean;
  saveError: string | null;
  canConnect: boolean;
  onCancel: () => void;
  onSave: (connect: boolean) => void;
}): React.JSX.Element {
  const { steps } = props;
  const index = steps.order.indexOf(steps.current);
  const last = steps.current === 'test';
  return (
    <footer className={styles.footer}>
      <Button variant="ghost" onClick={props.onCancel}>
        {t('cancel')}
      </Button>
      <span className={styles.hint}>
        {t('stepOf', {
          step: String(index + 1),
          count: String(steps.order.length),
        })}
      </span>
      {props.saveError && (
        <span className={styles.error} role="alert" title={props.saveError}>
          {props.saveError}
        </span>
      )}
      <span className={styles.spacer} />
      {index > 0 && <Button onClick={steps.back}>{t('back')}</Button>}
      {(last || props.editing) && (
        <Button
          type={last && !props.canConnect ? 'submit' : 'button'}
          variant={last && !props.canConnect ? 'primary' : 'secondary'}
          disabled={props.saving}
          onClick={(event) => {
            event.preventDefault();
            props.onSave(false);
          }}
        >
          {t('save')}
        </Button>
      )}
      {last && props.canConnect && (
        <Button
          type="submit"
          variant="primary"
          disabled={props.saving}
          onClick={(event) => {
            event.preventDefault();
            props.onSave(true);
          }}
        >
          {t('saveAndConnect')}
        </Button>
      )}
      {!last && (
        <Button type="submit" variant="primary" disabled={!props.ready}>
          {t('continue')}
          <Kbd variant="inline" aria-hidden="true">
            ↩
          </Kbd>
        </Button>
      )}
    </footer>
  );
}
