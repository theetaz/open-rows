// SPDX-License-Identifier: Apache-2.0
import { SelectField, fontFamily, useTheme } from '@openrows/ui';
import { FontRoles } from './FontRoles';
import { SettingRow } from './InterfaceSettings';
import styles from './Appearance.module.css';
import { rich, t } from './messages';
const sizeOptions = Array.from({ length: 8 }, (_, index) => ({
  value: String(index + 11),
  label: t('valuePx', { value: String(index + 11) }),
}));
export function TypographySettings(): React.JSX.Element {
  const { appearance: a, setAppearance } = useTheme();
  return (
    <section aria-label={t('typography')}>
      <h3>{t('typography')}</h3>
      <FontRoles />
      <div className={styles.settingsGroup}>
        <SettingRow
          title={t('interfaceFont')}
          description={t('workspaceControlsMenus')}
        >
          <SelectField
            aria-label={t('interfaceFont')}
            value={a.uiFont}
            options={[
              { value: 'system', label: t('systemSfPro') },
              { value: 'helvetica', label: t('helveticaNeue') },
              { value: 'arial', label: t('arial') },
              { value: 'verdana', label: t('verdana') },
            ]}
            onValueChange={(uiFont) => {
              setAppearance({ uiFont });
            }}
          />
          <SelectField
            aria-label={t('interfaceFontSize')}
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
          {rich('workspaceBeautifullyClear', {
            span: <span className={styles.previewBadge}>{t('connected')}</span>,
          })}
        </div>
        <SettingRow
          title={t('monospaceFont')}
          description={t('detailValuesOther')}
        >
          <SelectField
            aria-label={t('monospaceFont')}
            value={a.monoFont}
            options={[
              { value: 'system', label: t('systemSfMono') },
              { value: 'menlo', label: t('menlo') },
              { value: 'consolas', label: t('consolas') },
              { value: 'courier', label: t('courierNew') },
            ]}
            onValueChange={(monoFont) => {
              setAppearance({ monoFont });
            }}
          />
          <SelectField
            aria-label={t('monospaceFontSize')}
            value={String(a.monoSize)}
            options={sizeOptions}
            onValueChange={(size) => {
              setAppearance({ monoSize: Number(size) });
            }}
          />
        </SettingRow>
        <MonoPreview />
        <p className={styles.fontNote}>{t('fontsUseLocal')}</p>
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
        <span>{t('id')}</span>
        <span>{t('name')}</span>
        <span>{t('created')}</span>
      </div>
      <div>
        <span>1</span>
        <span>{t('sampleRow')}</span>
        <span>2026-01-01</span>
      </div>
      <p>
        {rich('bB2Public', { b: <b>{t('select')}</b>, b2: <b>{t('from')}</b> })}
      </p>
    </div>
  );
}
