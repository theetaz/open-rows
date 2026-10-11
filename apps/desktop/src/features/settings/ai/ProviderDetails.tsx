// SPDX-License-Identifier: Apache-2.0
import type { Capabilities, Verification } from '@openrows/ipc';
import styles from './Ai.module.css';
const supported = (value: boolean | null): string =>
  value === null ? 'unknown' : value ? 'yes' : 'no';
function capabilities(value: Capabilities): string {
  return `Streaming: ${supported(value.streaming)} · Tools: ${supported(value.toolCalling)} · Structured output: ${supported(value.structuredOutput)} · Vision: ${supported(value.vision)} · Context: ${String(value.contextWindow ?? 'unknown')} · Max output: ${String(value.maxOutput ?? 'unknown')}`;
}
/** Only reported model/usage facts; unsupported discovery stays unknown. */
export function ProviderDetails({
  result,
}: {
  result: Verification;
}): React.JSX.Element {
  return (
    <div className={styles.details}>
      <p>
        Usage (USD): {result.usage?.used ?? 'unknown'} · Limit:{' '}
        {result.usage?.limit ?? 'unknown'} · Remaining:{' '}
        {result.usage?.remaining ?? 'unknown'}
      </p>
      {result.models.length === 0 ? (
        <p>Available models: unknown</p>
      ) : (
        <ul aria-label="Available models">
          {result.models.map((model) => (
            <li key={model.id}>
              <strong>{model.name}</strong> <code>{model.id}</code>
              <p>{capabilities(model.capabilities)}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
