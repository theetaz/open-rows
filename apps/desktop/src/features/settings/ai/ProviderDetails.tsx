// SPDX-License-Identifier: Apache-2.0
import type { Capabilities, Verification } from '@openrows/ipc';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
const supported = (value: boolean | null): string =>
  value === null ? t('unknownValue') : value ? t('yes') : t('no');
function capabilities(value: Capabilities): string {
  return t('streamingStreamingTools', {
    streaming: supported(value.streaming),
    toolCalling: supported(value.toolCalling),
    structuredOutput: supported(value.structuredOutput),
    vision: supported(value.vision),
    contextWindow: String(value.contextWindow ?? t('unknownValue')),
    maxOutput: String(value.maxOutput ?? t('unknownValue')),
  });
}
/** Only reported model/usage facts; unsupported discovery stays unknown. */
export function ProviderDetails({
  result,
}: {
  result: Verification;
}): React.JSX.Element {
  return (
    <div className={styles.details}>
      {result.cli ? (
        <p>{t('usageReportedOnly')}</p>
      ) : (
        <p>
          {t('usageUsdUsed', {
            used: result.usage?.used ?? t('unknownValue'),
            limit: result.usage?.limit ?? t('unknownValue'),
            remaining: result.usage?.remaining ?? t('unknownValue'),
          })}
        </p>
      )}
      {result.models.length === 0 ? (
        <p>{t('availableModelsUnknown')}</p>
      ) : (
        <ul aria-label={t('availableModels')}>
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
