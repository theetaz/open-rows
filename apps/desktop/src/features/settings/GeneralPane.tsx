// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import type { GridDensity } from '@openrows/grid';
import {
  generalSettings,
  KEEPALIVE_CHOICES,
  setGridDensity,
  updateGeneral,
} from './general';
import type { Density } from './general';
import styles from '../appearance/Appearance.module.css';

const DENSITIES: readonly { value: Density; label: string }[] = [
  { value: 'compact', label: 'Compact' },
  { value: 'regular', label: 'Regular' },
  { value: 'comfortable', label: 'Comfortable' },
];

const GRID_DENSITIES: readonly { value: GridDensity; label: string }[] = [
  { value: 'compact', label: 'Compact' },
  { value: 'normal', label: 'Normal' },
  { value: 'comfortable', label: 'Comfortable' },
];

const keepaliveLabel = (seconds: number): string => {
  if (seconds === 0) return 'Off';
  return seconds < 60 ? `${String(seconds)} s` : `${String(seconds / 60)} min`;
};

const KEEPALIVE_OPTIONS = KEEPALIVE_CHOICES.map((seconds) => ({
  value: String(seconds),
  label: keepaliveLabel(seconds),
}));

/** Settings ▸ General: session restore, connection checks, density and motion. */
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
      <section aria-label="Connections">
        <h3>Connections</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Connection check"
            description="How often idle connections are checked. A lost connection is reopened before your next query; a running statement is never re-run."
          >
            <SegmentedControl
              aria-label="Connection check"
              options={KEEPALIVE_OPTIONS}
              value={String(general.keepaliveSeconds)}
              onValueChange={(seconds) => {
                const keepaliveSeconds = KEEPALIVE_CHOICES.find(
                  (choice) => String(choice) === seconds,
                );
                if (keepaliveSeconds !== undefined)
                  updateGeneral({ keepaliveSeconds });
              }}
            />
          </SettingRow>
        </div>
      </section>
      <LayoutSection />
    </>
  );
}

/** Sidebar and grid density, and reduced motion. */
function LayoutSection(): React.JSX.Element {
  const general = generalSettings.use();
  return (
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
          title="Grid rows"
          description="Row height of table and query result grids."
        >
          <SegmentedControl
            aria-label="Grid rows"
            options={GRID_DENSITIES}
            value={general.gridDensity}
            onValueChange={setGridDensity}
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
  );
}
