// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { cancelAiVerification } from '@openrows/ipc';
import type { AiError, ProviderConfig, Verification } from '@openrows/ipc';
import { useVerification } from './useVerification';
import { ProviderDetails } from './ProviderDetails';
import styles from './Ai.module.css';
const REASONS: Record<AiError['kind'], string> = {
  auth: 'Authentication failed',
  rateLimit: 'Rate limit reached',
  quota: 'Quota exhausted',
  network: 'Network failed',
  unsupported: 'Unsupported',
  invalidResponse: 'Invalid response',
  invalidConfig: 'Invalid endpoint',
  disabled: 'AI features are off',
  cancelled: 'Cancelled',
  timeout: 'Timed out',
};
function health(result: Verification | null): string {
  if (!result) return 'Configured';
  if (result.health.state !== 'failed')
    return result.health.state === 'verified' ? 'Verified' : 'Unknown';
  const reason = result.health.reason;
  const retry =
    reason.kind === 'rateLimit' && reason.retryAfter !== null
      ? ` (retry after ${String(reason.retryAfter)}s)`
      : '';
  return `Failed: ${REASONS[reason.kind]}${retry}`;
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
  return (
    <section aria-label={props.config.name} className={styles.provider}>
      <div className={styles.heading}>
        <strong>
          {props.config.name}
          {props.selected ? ' (default)' : ''}
        </strong>
        <span role="status">{busy ? 'Verifying…' : health(result)}</span>
      </div>
      <p>
        {props.config.kind} · {props.config.baseUrl}
      </p>
      <div className={styles.actions}>
        <Button
          size="sm"
          disabled={!props.enabled || busy}
          onClick={() => {
            void verify();
          }}
        >
          Verify
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
      {result && <ProviderDetails result={result} />}
    </section>
  );
}
