// SPDX-License-Identifier: Apache-2.0
import { ProviderAllowlist, ProductionConsent } from './SharingOptions';
import { SelectField } from '@openrows/ui';
import type { ProviderConfig, SharingPolicy } from '@openrows/ipc';
import styles from './Ai.module.css';
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
        label="Sharing level"
        value={policy.level}
        options={[
          { value: 'none', label: 'None (default)' },
          { value: 'schemaOnly', label: 'Schema only' },
          { value: 'schemaAndStats', label: 'Schema and statistics' },
          { value: 'sampleRows', label: 'Sample rows' },
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
        Local models only
      </label>
      <small>
        Local endpoints must use a local protocol on loopback. Their inference
        location remains unverified.
      </small>
      <label>
        <input
          type="checkbox"
          checked={policy.telemetry}
          onChange={(event) => {
            onChange({ ...policy, telemetry: event.target.checked });
          }}
        />{' '}
        Share aggregate statistics and plans
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
