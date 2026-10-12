// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, setGridDensity } from './general';
import { tableSettings, updateTableSettings } from './tableSettings';
import type { GridScrollbars, RowCountMode } from './tableSettings';
import { CopySection } from './CopySection';
import styles from '../appearance/Appearance.module.css';

const SCROLLBARS: readonly { value: GridScrollbars; label: string }[] = [
  { value: 'quiet', label: 'Show when hovered' },
  { value: 'always', label: 'Always visible' },
];
const COUNTS: readonly { value: RowCountMode; label: string }[] = [
  { value: 'estimate', label: 'Estimated (~)' },
  { value: 'exact', label: 'Exact' },
];

/** Shared table presentation (FR-GRID-12); tabs can still toggle stripes and counts. */
export function TablesPane(): React.JSX.Element {
  const general = generalSettings.useValue();
  const tables = tableSettings.useValue();
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
            title="Alternate row backgrounds"
            description="Stripe table and result rows; a table's display menu can still toggle its own."
          >
            <Switch
              aria-label="Alternate row backgrounds"
              checked={tables.striped}
              onCheckedChange={(striped) => {
                updateTableSettings({ striped });
              }}
            />
          </SettingRow>
          <SettingRow
            title="Row count"
            description="Estimated shows the planner's ~count with a Count action. Exact counts when filters change or rows reload; you can cancel."
          >
            <SelectField
              aria-label="Row count"
              value={tables.rowCount}
              options={COUNTS}
              onValueChange={(rowCount) => {
                updateTableSettings({ rowCount });
              }}
            />
          </SettingRow>
          <SettingRow
            title="Scrollbars"
            description="Grid scrollbars fade until hovered, or stay visible."
          >
            <SelectField
              aria-label="Grid scrollbars"
              value={tables.scrollbars}
              options={SCROLLBARS}
              onValueChange={(scrollbars) => {
                updateTableSettings({ scrollbars });
              }}
            />
          </SettingRow>
        </div>
      </section>
      <CopySection />
    </>
  );
}
