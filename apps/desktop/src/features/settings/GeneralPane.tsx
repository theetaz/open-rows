// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, updateGeneral } from './general';
import type { Density } from './general';
import styles from '../appearance/Appearance.module.css';

const DENSITIES: readonly { value: Density; label: string }[] = [
  { value: 'compact', label: 'Compact' },
  { value: 'regular', label: 'Regular' },
  { value: 'comfortable', label: 'Comfortable' },
];

/** Settings ▸ General: session restore, density and motion. */
export function GeneralPane(): React.JSX.Element {
  const general = generalSettings.use();
  return (
    <>
      <section aria-label="Session">
        <h3>Session</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Restore session on launch"
            description="Reopen the previous connections, databases and tabs. Production and confirm-before-write connections wait for you to connect."
          >
            <Switch
              aria-label="Restore session on launch"
              checked={general.restoreSession}
              onCheckedChange={(restoreSession) => {
                updateGeneral({ restoreSession });
              }}
            />
          </SettingRow>
        </div>
      </section>
      <section aria-label="Layout">
        <h3>Layout</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Density"
            description="Row height of the sidebar's object tree and lists."
          >
            <SegmentedControl
              aria-label="Density"
              options={DENSITIES}
              value={general.density}
              onValueChange={(density) => {
                updateGeneral({ density });
              }}
            />
          </SettingRow>
          <SettingRow
            title="Reduce motion"
            description="Turn off panel, popover and disclosure animations. The system setting always applies."
          >
            <Switch
              aria-label="Reduce motion"
              checked={general.reduceMotion}
              onCheckedChange={(reduceMotion) => {
                updateGeneral({ reduceMotion });
              }}
            />
          </SettingRow>
        </div>
      </section>
    </>
  );
}
