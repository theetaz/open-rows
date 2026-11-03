// SPDX-License-Identifier: Apache-2.0
import { SegmentedControl, SelectField, Switch } from '@openrows/ui';
import { secondsLabel } from '../connections/limitsForm';
import { SettingRow } from '../appearance/InterfaceSettings';
import type { GridDensity } from '@openrows/grid';
import {
  generalSettings,
  KEEPALIVE_CHOICES,
  setGridDensity,
  STATEMENT_TIMEOUT_CHOICES,
  updateGeneral,
} from './general';
import type { Density } from './general';
import styles from '../appearance/Appearance.module.css';
import { HistorySection } from './HistorySection';
import { CopySection } from './CopySection';
import { StartupSettings } from './StartupSettings';
import { t } from './messages/lazyGeneral';

const DENSITIES: readonly { value: Density; label: string }[] = [
  { value: 'compact', label: t('compact') },
  { value: 'regular', label: t('regular') },
  { value: 'comfortable', label: t('comfortable') },
];

const GRID_DENSITIES: readonly { value: GridDensity; label: string }[] = [
  { value: 'compact', label: t('compact') },
  { value: 'normal', label: t('normal') },
  { value: 'comfortable', label: t('comfortable') },
];

const keepaliveLabel = (seconds: number): string => {
  if (seconds === 0) return t('off');
  return seconds < 60 ? `${String(seconds)} s` : `${String(seconds / 60)} min`;
};

const KEEPALIVE_OPTIONS = KEEPALIVE_CHOICES.map((seconds) => ({
  value: String(seconds),
  label: keepaliveLabel(seconds),
}));

const STATEMENT_TIMEOUT_OPTIONS = STATEMENT_TIMEOUT_CHOICES.map((seconds) => ({
  value: String(seconds),
  label: seconds === 0 ? t('noLimit') : secondsLabel(seconds),
}));

/**
 * Settings ▸ General: session restore, connection checks, the default
 * statement timeout, density and motion.
 */
export function GeneralPane(): React.JSX.Element {
  const general = generalSettings.useValue();
  return (
    <>
      <section aria-label={t('session')}>
        <h3>{t('session')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title={t('restoreSessionLaunch')}
            description={t('reopenPreviousConnections')}
          >
            <Switch
              aria-label={t('restoreSessionLaunch')}
              checked={general.restoreSession}
              onCheckedChange={(restoreSession) => {
                updateGeneral({ restoreSession });
              }}
            />
          </SettingRow>
        </div>
      </section>
      <StartupSettings />
      <ConnectionSettings />
      <HistorySection />
      <section aria-label={t('layout')}>
        <h3>{t('layout')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow title={t('density')} description={t('rowHeightSidebar')}>
            <SegmentedControl
              aria-label={t('density')}
              options={DENSITIES}
              value={general.density}
              onValueChange={(density) => {
                updateGeneral({ density });
              }}
            />
          </SettingRow>
          <SettingRow title={t('gridRows')} description={t('rowHeightTable')}>
            <SegmentedControl
              aria-label={t('gridRows')}
              options={GRID_DENSITIES}
              value={general.gridDensity}
              onValueChange={setGridDensity}
            />
          </SettingRow>
          <SettingRow title={t('reduceMotion')} description={t('turnOffPanel')}>
            <Switch
              aria-label={t('reduceMotion')}
              checked={general.reduceMotion}
              onCheckedChange={(reduceMotion) => {
                updateGeneral({ reduceMotion });
              }}
            />
          </SettingRow>
        </div>
      </section>
      <CopySection />
    </>
  );
}

/** Connection checks and the default statement timeout. */
function ConnectionSettings(): React.JSX.Element {
  const general = generalSettings.useValue();
  return (
    <section aria-label={t('connections')}>
      <h3>{t('connections')}</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title={t('connectionCheck')}
          description={t('howOftenIdle')}
        >
          <SegmentedControl
            aria-label={t('connectionCheck')}
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
        <SettingRow
          title={t('statementTimeout')}
          description={t('cancelStatementRuns')}
        >
          <SelectField
            aria-label={t('defaultStatementTimeout')}
            value={String(general.statementTimeoutSeconds)}
            options={STATEMENT_TIMEOUT_OPTIONS}
            onValueChange={(seconds) => {
              const statementTimeoutSeconds = STATEMENT_TIMEOUT_CHOICES.find(
                (choice) => String(choice) === seconds,
              );
              if (statementTimeoutSeconds !== undefined)
                updateGeneral({ statementTimeoutSeconds });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
