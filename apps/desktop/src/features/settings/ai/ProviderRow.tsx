// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { cancelAiVerification } from '@openrows/ipc';
import type { ProviderConfig, Verification } from '@openrows/ipc';
import { useVerification } from './useVerification';
import { ProviderDetails } from './ProviderDetails';
import { REASONS } from './aiReasons';
import { isCliProvider } from './aiSettings';
import { CodexSignIn } from './CodexSignIn';
import { codexLocation, codexStatusText } from './codexStatus';
import styles from './Ai.module.css';
function health(result: Verification | null): string {
  if (!result) return 'Configured';
  if (result.cli) return codexStatusText(result.cli);
  if (result.health.state !== 'failed')
    return result.health.state === 'verified' ? 'Verified' : 'Unknown';
  const reason = result.health.reason;
  const retry =
    reason.kind === 'rateLimit' && reason.retryAfter !== null
      ? ` (retry after ${String(reason.retryAfter)}s)`
      : '';
  return `Failed: ${REASONS[reason.kind]}${retry}`;
}
/** Installed CLIs are checked for discovery/sign-in; API providers are verified. */
function labels(config: ProviderConfig): {
  action: string;
  running: string;
  where: string;
} {
  return isCliProvider(config.kind)
    ? {
        action: 'Check status',
        running: 'Checking…',
        where: codexLocation(config.baseUrl),
      }
    : {
        action: 'Verify',
        running: 'Verifying…',
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
    <section aria-label={props.config.name} className={styles.provider}>
      <div className={styles.heading}>
        <strong>
          {props.config.name}
          {props.selected ? ' (default)' : ''}
        </strong>
        <span role="status">{busy ? words.running : health(result)}</span>
      </div>
      <p>{words.where}</p>
      <div className={styles.actions}>
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
            Cancel verification
          </Button>
        )}
        <Button
          size="sm"
          disabled={busy || props.disabled}
          onClick={props.onEdit}
        >
          Edit
        </Button>
        <Button
          size="sm"
          disabled={busy || props.disabled}
          onClick={props.onRemove}
        >
          Remove
        </Button>
        <Button
          size="sm"
          disabled={props.selected || busy || props.disabled}
          onClick={props.onDefault}
        >
          Use as default
        </Button>
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
    </section>
  );
}
