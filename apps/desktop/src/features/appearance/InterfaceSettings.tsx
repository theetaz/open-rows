// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, useTheme } from '@openrows/ui';
import type { ReactNode } from 'react';
import { HighlightMatch } from '../settings/HighlightMatch';
import { useSettingMatch } from '../settings/settingsSearch';
import styles from './Appearance.module.css';
import { t } from './messages';
/** One titled setting; hidden while the Settings search does not match it. */
export function SettingRow({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}): React.JSX.Element | null {
  const shown = useSettingMatch(title, description);
  if (!shown) return null;
  return (
    <div className={styles.settingRow} data-setting-row>
      <div>
        <strong>
          <HighlightMatch text={title} />
        </strong>
        <p>
          <HighlightMatch text={description} />
        </p>
      </div>
      <div className={styles.controls}>{children}</div>
    </div>
  );
}
function Slider({
  label,
  value,
  min,
  max,
  unit,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  unit: string;
  onChange: (value: number) => void;
}): React.JSX.Element {
  return (
    <>
      <output>
        {value}
        {unit}
      </output>
      <input
        aria-label={label}
        type="range"
        min={min}
        max={max}
        step={unit === ' ms' ? 20 : 5}
        value={value}
        onChange={(event) => {
          onChange(Number(event.target.value));
        }}
      />
    </>
  );
}
export function InterfaceSettings(): React.JSX.Element {
  const { appearance: a, setAppearance } = useTheme();
  return (
    <>
      <section aria-label={t('interface')}>
        <h3>{t('interface')}</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title={t('contrast')}
            description={t('adjustSecondaryText')}
          >
            <Slider
              label={t('contrast')}
              value={a.contrast}
              min={80}
              max={150}
              unit="%"
              onChange={(contrast) => {
                setAppearance({ contrast });
              }}
            />
          </SettingRow>
          <SettingRow
            title={t('glassOpacity')}
            description={t('higherValuesMake')}
          >
            <Button
              size="sm"
              variant="ghost"
              aria-label={t('resetGlassOpacity')}
              onClick={() => {
                setAppearance({ opacity: 95 });
              }}
            >
              ↺
            </Button>
            <Slider
              label={t('glassOpacity')}
              value={a.opacity}
              min={50}
              max={100}
              unit="%"
              onChange={(opacity) => {
                setAppearance({ opacity });
              }}
            />
          </SettingRow>
          <SettingRow
            title={t('changeColors')}
            description={t('distinguishInsertedDeleted')}
          >
            <SelectField
              aria-label={t('changeColors')}
              value={a.changeColors}
              options={[
                { value: 'red-green', label: t('redGreen') },
                { value: 'blue-orange', label: t('blueOrange') },
              ]}
              onValueChange={(changeColors) => {
                setAppearance({ changeColors });
              }}
            />
          </SettingRow>
          <div className={styles.changePreview}>
            <span data-kind="insert">{t('insertedRow')}</span>
            <span data-kind="delete">{t('deletedRow')}</span>
            <span data-kind="edit">{t('editedCell')}</span>
          </div>
        </div>
      </section>
      <MotionSettings />
    </>
  );
}

function MotionSettings(): React.JSX.Element {
  const { appearance: a, setAppearance } = useTheme();
  return (
    <section aria-label={t('motion')}>
      <h3>{t('motion')}</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title={t('panelAnimations')}
          description={t('controlHowQuickly')}
        >
          <Slider
            label={t('panelAnimations')}
            value={a.motion}
            min={0}
            max={400}
            unit=" ms"
            onChange={(motion) => {
              setAppearance({ motion });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
