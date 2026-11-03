// SPDX-License-Identifier: Apache-2.0
import { ProviderAllowlist, ProductionConsent } from './SharingOptions';
import { SelectField } from '@openrows/ui';
import type { ProviderConfig, SharingPolicy } from '@openrows/ipc';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
type Props = {
  policy: Required<SharingPolicy>;
  providers: readonly ProviderConfig[];
  production: boolean;
  onChange: (policy: Required<SharingPolicy>) => void;
};
/** Explicit category, routing and separate telemetry controls for one connection. */
export function SharingControls({
  policy,
  providers,
  production,
  onChange,
}: Props): React.JSX.Element {
  return (
    <div className={styles.form}>
      <SelectField<SharingPolicy['level'] & string>
        label={t('sharingLevel')}
        value={policy.level}
        options={[
          { value: 'none', label: t('noneDefault') },
          { value: 'schemaOnly', label: t('schemaOnly') },
          { value: 'schemaAndStats', label: t('schemaStatistics') },
          { value: 'sampleRows', label: t('sampleRows') },
        ]}
        onValueChange={(level) => {
          onChange({ ...policy, level });
        }}
      />
      <label>
        <input
          type="checkbox"
          checked={policy.localOnly}
          onChange={(event) => {
            onChange({ ...policy, localOnly: event.target.checked });
          }}
        />{' '}
        {t('localModelsOnly')}
      </label>
      <small>{t('localEndpointsMust')}</small>
      <label>
        <input
          type="checkbox"
          checked={policy.telemetry}
          onChange={(event) => {
            onChange({ ...policy, telemetry: event.target.checked });
          }}
        />{' '}
        {t('shareAggregateStatistics')}
      </label>
      <ProviderAllowlist
        selected={policy.providers}
        providers={providers}
        onChange={(ids) => {
          onChange({ ...policy, providers: ids });
        }}
      />
      {production && (
        <ProductionConsent
          confirmed={policy.productionConfirmed}
          onChange={(confirmed) => {
            onChange({ ...policy, productionConfirmed: confirmed });
          }}
        />
      )}
    </div>
  );
}
