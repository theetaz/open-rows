// SPDX-License-Identifier: Apache-2.0
import { SelectField, useTheme } from '@openrows/ui';
import { SettingRow } from './InterfaceSettings';
import { t } from './messages';

const roles = ['editor', 'grid', 'sidebar'] as const;
const monospace = ['system', 'menlo', 'consolas', 'courier'];
const sizes = [11, 12, 13, 14, 15, 16, 18, 20, 24];

/** Independent token metrics, updated only when preferences change. */
export function FontRoles(): React.JSX.Element {
  const { appearance, setAppearance } = useTheme();
  return (
    <>
      {roles.map((role) => (
        <SettingRow
          key={role}
          title={t('roleFont', { role })}
          description={t('independentRoleFamily', { role })}
        >
          <SelectField
            aria-label={t('roleFont', { role })}
            value={appearance[`${role}Font`]}
            options={(role === 'sidebar'
              ? ['system', 'helvetica', 'arial', 'verdana']
              : monospace
            ).map((value) => ({ value, label: value }))}
            onValueChange={(font) => {
              setAppearance({ [`${role}Font`]: font });
            }}
          />
          <SelectField
            aria-label={t('roleFontSize', { role })}
            value={String(appearance[`${role}Size`])}
            options={sizes.map((size) => ({
              value: String(size),
              label: t('sizePx', { size: String(size) }),
            }))}
            onValueChange={(size) => {
              setAppearance({ [`${role}Size`]: Number(size) });
            }}
          />
        </SettingRow>
      ))}
      <SettingRow
        title={t('editorLineHeight')}
        description={t('verticalSpacingSql')}
      >
        <SelectField
          aria-label={t('editorLineHeight')}
          value={String(appearance.editorLineHeight)}
          options={[1.2, 1.5, 1.8, 2].map((height) => ({
            value: String(height),
            label: String(height),
          }))}
          onValueChange={(height) => {
            setAppearance({ editorLineHeight: Number(height) });
          }}
        />
      </SettingRow>
    </>
  );
}
