// SPDX-License-Identifier: Apache-2.0
import type { ProviderConfig } from '@openrows/ipc';
import styles from './Ai.module.css';
/** Only saved providers are available for a connection allowlist. */
export function ProviderAllowlist(props: {
  selected: string[];
  providers: readonly ProviderConfig[];
  onChange: (ids: string[]) => void;
}): React.JSX.Element {
  return (
    <fieldset>
      <legend>Allowed providers</legend>
      {props.providers.length === 0 && (
        <p>Add a provider in Settings → AI first.</p>
      )}
      {props.providers.map((provider) => (
        <label key={provider.id} className={styles.heading}>
          <span>{provider.name}</span>
          <input
            aria-label={`Allow ${provider.name}`}
            type="checkbox"
            checked={props.selected.includes(provider.id)}
            onChange={(event) => {
              props.onChange(
                event.target.checked
                  ? [...props.selected, provider.id]
                  : props.selected.filter((id) => id !== provider.id),
              );
            }}
          />
        </label>
      ))}
    </fieldset>
  );
}
/** Saving production sharing always requires a visible acknowledgement. */
export function ProductionConsent(props: {
  confirmed: boolean;
  onChange: (confirmed: boolean) => void;
}): React.JSX.Element {
  return (
    <div>
      <p role="note">
        Production: sharing is off until you explicitly confirm. The selected
        providers may receive database context.
      </p>
      <label>
        <input
          type="checkbox"
          checked={props.confirmed}
          onChange={(event) => {
            props.onChange(event.target.checked);
          }}
        />{' '}
        I confirm sharing production context with the allowed providers
      </label>
    </div>
  );
}
