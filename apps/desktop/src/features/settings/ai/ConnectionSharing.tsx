// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { useSharingPolicy } from './useSharingPolicy';
import { aiSettings } from './aiSettings';
import { SharingControls } from './SharingControls';
import { RedactionControls } from './RedactionControls';
import { ContextPreview } from './ContextPreview';
import styles from './Ai.module.css';
/** Connection settings editor backed by native policy storage, separate from portable profiles. */
export default function ConnectionSharing(props: {
  id: string;
  production: boolean;
}): React.JSX.Element {
  const settings = aiSettings.useValue();
  const { policy, dirty, busy, failure, saved, change, save } =
    useSharingPolicy(props.id);
  return (
    <section
      aria-label="Connection data sharing"
      aria-busy={busy}
      className={styles.provider}
    >
      {policy && (
        <fieldset disabled={busy}>
          <legend>AI data sharing</legend>
          <SharingControls
            policy={policy}
            providers={settings.providers}
            production={props.production}
            onChange={change}
          />
          <RedactionControls
            rules={policy.rules}
            onChange={(rules) => {
              change({ ...policy, rules });
            }}
          />
          <Button
            type="button"
            disabled={
              !dirty ||
              (props.production &&
                policy.level !== 'none' &&
                !policy.productionConfirmed)
            }
            onClick={() => {
              void save();
            }}
          >
            Save sharing policy
          </Button>
          {saved && <p role="status">Sharing policy saved.</p>}
          <ContextPreview
            key={JSON.stringify(policy)}
            id={props.id}
            providers={settings.providers}
            disabled={dirty || !settings.enabled}
          />
          {!settings.enabled && (
            <p className={styles.notice}>
              AI features are off. Enable them in Settings → AI to preview
              authorised context.
            </p>
          )}
        </fieldset>
      )}
      {failure && <p role="alert">{failure}</p>}
    </section>
  );
}
