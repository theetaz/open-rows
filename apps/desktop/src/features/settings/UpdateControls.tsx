// SPDX-License-Identifier: Apache-2.0
import { Button } from '@openrows/ui';
import { generalSettings } from './general';
import { updateFeed } from './updateFeed';
import { checkUpdate, useUpdate } from './updater';

/** Manual checks use the same consent and release channel as launch checks. */
export function UpdateControls(): React.JSX.Element {
  const general = generalSettings.useValue();
  const state = useUpdate();
  return (
    <div>
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
        Check for updates
      </Button>
      <p role="status">{state.message}</p>
    </div>
  );
}
