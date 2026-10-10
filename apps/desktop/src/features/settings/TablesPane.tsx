// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, setGridDensity } from './general';
import { CopySection } from './CopySection';
import styles from '../appearance/Appearance.module.css';

/** Shared table presentation, retaining explicit per-tab count and striping controls. */
export function TablesPane(): React.JSX.Element {
  const general = generalSettings.use();
  return (
    <>
      <section aria-label="Tables">
        <h3>Tables</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Table row density"
            description="Compact, default or comfortable row height; changes apply to mounted grids."
          >
            <SelectField
              aria-label="Table row density"
              value={general.gridDensity}
              options={[
                { value: 'compact', label: 'Compact' },
                { value: 'normal', label: 'Default' },
                { value: 'comfortable', label: 'Comfortable' },
              ]}
              onValueChange={setGridDensity}
            />
          </SettingRow>
          <SettingRow
            title="Row striping and row numbers"
            description="Use the table footer to toggle alternating stripes and the number gutter independently per tab."
          >
            <span>Table footer controls</span>
          </SettingRow>
          <SettingRow
            title="Estimated and exact counts"
            description="Estimates appear with ~; Count requests an exact count explicitly. Browsing never counts silently."
          >
            <span>Count in table footer</span>
          </SettingRow>
          <SettingRow
            title="Scrollbars"
            description="Native scrollbars follow your operating system accessibility preferences."
          >
            <span>System preference</span>
          </SettingRow>
        </div>
      </section>
      <CopySection />
    </>
  );
}
