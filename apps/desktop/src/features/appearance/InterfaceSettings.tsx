// SPDX-License-Identifier: Apache-2.0
import { Button, SelectField, useTheme } from '@openrows/ui';
import type { ReactNode } from 'react';
import { HighlightMatch } from '../settings/HighlightMatch';
import { useSettingMatch } from '../settings/settingsSearch';
import styles from './Appearance.module.css';
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
      <section aria-label="Interface">
        <h3>Interface</h3>
        <div className={styles.settingsGroup}>
          <SettingRow
            title="Contrast"
            description="Adjust secondary text and borders across the interface."
          >
            <Slider
              label="Contrast"
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
            title="Glass opacity"
            description="Higher values make glass panels and menus more solid."
          >
            <Button
              size="sm"
              variant="ghost"
              aria-label="Reset glass opacity"
              onClick={() => {
                setAppearance({ opacity: 95 });
              }}
            >
              ↺
            </Button>
            <Slider
              label="Glass opacity"
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
            title="Change colors"
            description="Distinguish inserted and deleted rows. Labels remain visible."
          >
            <SelectField
              aria-label="Change colors"
              value={a.changeColors}
              options={[
                { value: 'red-green', label: 'Red & green' },
                { value: 'blue-orange', label: 'Blue & orange' },
              ]}
              onValueChange={(changeColors) => {
                setAppearance({ changeColors });
              }}
            />
          </SettingRow>
          <div className={styles.changePreview}>
            <span data-kind="insert">+ Inserted row</span>
            <span data-kind="delete">− Deleted row</span>
            <span data-kind="edit">Edited cell</span>
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
    <section aria-label="Motion">
      <h3>Motion</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title="Panel animations"
          description="Control how quickly sidebars open and close. Zero is instant."
        >
          <Slider
            label="Panel animations"
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
