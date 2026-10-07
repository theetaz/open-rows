// SPDX-License-Identifier: Apache-2.0
import { SelectField, fontFamily, useTheme } from '@openrows/ui';
import { SettingRow } from './InterfaceSettings';
import styles from './Appearance.module.css';
const sizeOptions = Array.from({ length: 8 }, (_, index) => ({
  value: String(index + 11),
  label: `${String(index + 11)} px`,
}));
export function TypographySettings(): React.JSX.Element {
  const { appearance: a, setAppearance } = useTheme();
  return (
    <section aria-label="Typography">
      <h3>Typography</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title="Interface font"
          description="Workspace controls, menus, tabs and buttons."
        >
          <SelectField
            aria-label="Interface font"
            value={a.uiFont}
            options={[
              { value: 'system', label: 'System (SF Pro on macOS)' },
              { value: 'helvetica', label: 'Helvetica Neue' },
              { value: 'arial', label: 'Arial' },
              { value: 'verdana', label: 'Verdana' },
            ]}
            onValueChange={(uiFont) => {
              setAppearance({ uiFont });
            }}
          />
          <SelectField
            aria-label="Interface font size"
            value={String(a.uiSize)}
            options={sizeOptions}
            onValueChange={(size) => {
              setAppearance({ uiSize: Number(size) });
            }}
          />
        </SettingRow>
        <div
          className={styles.typePreview}
          style={{ fontFamily: fontFamily('ui', a.uiFont), fontSize: a.uiSize }}
        >
          Your workspace, beautifully clear.{' '}
          <span className={styles.previewBadge}>Connected</span>
        </div>
        <SettingRow
          title="Monospace font"
          description="Table names, column names, data, SQL and detail values."
        >
          <SelectField
            aria-label="Monospace font"
            value={a.monoFont}
            options={[
              { value: 'system', label: 'System (SF Mono on macOS)' },
              { value: 'menlo', label: 'Menlo' },
              { value: 'consolas', label: 'Consolas' },
              { value: 'courier', label: 'Courier New' },
            ]}
            onValueChange={(monoFont) => {
              setAppearance({ monoFont });
            }}
          />
          <SelectField
            aria-label="Monospace font size"
            value={String(a.monoSize)}
            options={sizeOptions}
            onValueChange={(size) => {
              setAppearance({ monoSize: Number(size) });
            }}
          />
        </SettingRow>
        <MonoPreview />
        <p className={styles.fontNote}>
          Fonts use local system fallbacks when the chosen face is unavailable.
        </p>
      </div>
    </section>
  );
}

function MonoPreview(): React.JSX.Element {
  const { appearance: a } = useTheme();
  return (
    <div
      className={styles.monoPreview}
      style={{
        fontFamily: fontFamily('mono', a.monoFont),
        fontSize: a.monoSize,
      }}
    >
      <div>
        <span>id</span>
        <span>name</span>
        <span>created_at</span>
      </div>
      <div>
        <span>1</span>
        <span>sample_row</span>
        <span>2026-01-01</span>
      </div>
      <p>
        <b>SELECT</b> * <b>FROM</b> public.example;
      </p>
    </div>
  );
}
