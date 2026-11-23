// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { generalSettings } from './general';
import { updateFeed } from './updateFeed';
import { checkUpdate, useUpdate } from './updater';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyGeneral';

/** Manual checks use the same consent and release channel as launch checks. */
export function UpdateControls(): React.JSX.Element {
  const general = generalSettings.useValue();
  const state = useUpdate();
  return (
    <div className={styles.inlineAction}>
      <Button
        disabled={
          !general.updateNotifications ||
          !updateFeed(general.updateChannel) ||
          state.checking
        }
        onClick={() => {
          void checkUpdate();
        }}
      >
        {t('checkUpdates')}
      </Button>
      <p role="status">{state.message}</p>
    </div>
  );
}
