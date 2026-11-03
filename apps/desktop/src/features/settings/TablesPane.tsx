// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { generalSettings, setGridDensity } from './general';
import { tableSettings, updateTableSettings } from './tableSettings';
import type { GridScrollbars, RowCountMode } from './tableSettings';
import { CopySection } from './CopySection';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyPanes';

const SCROLLBARS: readonly { value: GridScrollbars; label: string }[] = [
  { value: 'quiet', label: t('showWhenHovered') },
  { value: 'always', label: t('alwaysVisible') },
];
const COUNTS: readonly { value: RowCountMode; label: string }[] = [
  { value: 'estimate', label: t('estimated') },
  { value: 'exact', label: t('exact') },
];

/** Shared table presentation (FR-GRID-12); tabs can still toggle stripes and counts. */
export function TablesPane(): React.JSX.Element {
  const general = generalSettings.useValue();
  const tables = tableSettings.useValue();
  return (
    <>
      <section aria-label={t('tables')}>
        <h3>{t('tables')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title={t('tableRowDensity')}
            description={t('compactDefaultComfortable')}
          >
            <SelectField
              aria-label={t('tableRowDensity')}
              value={general.gridDensity}
              options={[
                { value: 'compact', label: t('compact') },
                { value: 'normal', label: t('default') },
                { value: 'comfortable', label: t('comfortable') },
              ]}
              onValueChange={setGridDensity}
            />
          </SettingRow>
          <SettingRow
            title={t('alternateRowBackgrounds')}
            description={t('stripeTableResult')}
          >
            <Switch
              aria-label={t('alternateRowBackgrounds')}
              checked={tables.striped}
              onCheckedChange={(striped) => {
                updateTableSettings({ striped });
              }}
            />
          </SettingRow>
          <SettingRow
            title={t('rowCount')}
            description={t('estimatedShowsPlanner')}
          >
            <SelectField
              aria-label={t('rowCount')}
              value={tables.rowCount}
              options={COUNTS}
              onValueChange={(rowCount) => {
                updateTableSettings({ rowCount });
              }}
            />
          </SettingRow>
          <SettingRow
            title={t('scrollbars')}
            description={t('gridScrollbarsFade')}
          >
            <SelectField
              aria-label={t('gridScrollbars')}
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
