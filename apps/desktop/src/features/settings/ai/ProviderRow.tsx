// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { cancelAiVerification } from '@openrows/ipc';
import type { ProviderConfig, Verification } from '@openrows/ipc';
import { useVerification } from './useVerification';
import { ProviderDetails } from './ProviderDetails';
import { REASONS } from './aiReasons';
import { isCliProvider } from './aiSettings';
import { CodexSignIn } from './CodexSignIn';
import { ProviderKey } from './ProviderKey';
import { codexLocation, codexStatusText } from './codexStatus';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
function health(result: Verification | null): string {
  if (!result) return t('configured');
  if (result.cli) return codexStatusText(result.cli);
  if (result.health.state !== 'failed')
    return result.health.state === 'verified' ? t('verified') : t('unknown');
  const reason = result.health.reason;
  return reason.kind === 'rateLimit' && reason.retryAfter !== null
    ? t('failedRetryAfter', {
        reason: REASONS[reason.kind],
        retryAfter: String(reason.retryAfter),
      })
    : t('failedReason', { reason: REASONS[reason.kind] });
}
/** Installed CLIs are checked for discovery/sign-in; API providers are verified. */
function labels(config: ProviderConfig): {
  action: string;
  running: string;
  where: string;
} {
  return isCliProvider(config.kind)
    ? {
        action: t('checkStatus'),
        running: t('checking'),
        where: codexLocation(config.baseUrl),
      }
    : {
        action: t('verify'),
        running: t('verifying'),
        where: `${config.kind} · ${config.baseUrl}`,
      };
}
type Props = {
  config: ProviderConfig;
  enabled: boolean;
  selected: boolean;
  disabled: boolean;
  onEdit: () => void;
  onRemove: () => void;
  onDefault: () => void;
};
/** Explicit verify action with cancellation; no automatic network calls on mount. */
export function ProviderRow(props: Props): React.JSX.Element {
  const { result, busy, failure, verify } = useVerification(props.config.id);
  const words = labels(props.config);
  return (
    <section
      aria-label={props.config.name}
      className={[styles.row, styles.provider].join(' ')}
    >
      <span className={styles.monogram} aria-hidden="true">
        {props.config.name.slice(0, 2)}
      </span>
      <div className={styles.rowBody}>
        <strong>
          {props.selected
            ? t('nameDefault', { name: props.config.name })
            : props.config.name}
        </strong>
        <p>{words.where}</p>
      </div>
      <div className={styles.accessory}>
        <span role="status">{busy ? words.running : health(result)}</span>
        <Button
          size="sm"
          disabled={!props.enabled || busy}
          onClick={() => {
            void verify();
          }}
        >
          {words.action}
        </Button>
        {busy && (
          <Button
            size="sm"
            onClick={() => {
              void cancelAiVerification(props.config.id).catch(() => undefined);
            }}
          >
            {t('cancelVerification')}
          </Button>
        )}
      </div>
      <ProviderMore
        {...props}
        busy={busy}
        failure={failure}
        result={result}
        verify={verify}
      />
    </section>
  );
}

/** Edit, remove, default and key actions, then outcomes and reported details. */
function ProviderMore(
  props: Props & {
    busy: boolean;
    failure: string;
    result: Verification | null;
    verify: () => Promise<void>;
  },
): React.JSX.Element {
  const { busy, failure, result, verify } = props;
  return (
    <div className={styles.rowFull}>
      <div className={styles.actions}>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy || props.disabled}
          onClick={props.onEdit}
        >
          {t('edit')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={busy || props.disabled}
          onClick={props.onRemove}
        >
          {t('remove')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={props.selected || busy || props.disabled}
          onClick={props.onDefault}
        >
          {t('useDefault')}
        </Button>
        <ProviderKey
          config={props.config}
          busy={busy}
          disabled={props.disabled}
        />
      </div>
      {failure && <p role="alert">{failure}</p>}
      {result?.cli?.state === 'signedOut' && (
        <CodexSignIn
          id={props.config.id}
          enabled={props.enabled && !busy}
          onDone={() => {
            void verify();
          }}
        />
      )}
      {result && <ProviderDetails result={result} />}
    </div>
  );
}
