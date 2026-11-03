// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { useEditorSettings } from '../query/editorSettings';
import {
  AUTO_LIMITS,
  executionSettings,
  updateExecution,
} from '../query/executionSettings';
import type { ExecutionSettings } from '../query/executionSettings';
import styles from '../appearance/Appearance.module.css';
import { t } from './messages/lazyPanes';

type Flag = 'runSelection' | 'confirmMultiple' | 'showNotices';

const FLAGS: readonly { key: Flag; title: string; description: string }[] = [
  {
    key: 'runSelection',
    title: t('runSelection'),
    description: t('textSelectedRuns'),
  },
  {
    key: 'confirmMultiple',
    title: t('confirmMultipleStatements'),
    description: t('askBeforeRun'),
  },
  {
    key: 'showNotices',
    title: t('showServerNotices'),
    description: t('listNoticesWarnings'),
  },
];

const limitLabel = (limit: ExecutionSettings['autoLimit']): string =>
  limit === null ? t('off') : `LIMIT ${limit.toLocaleString('en-US')}`;

/** Settings ▸ Execution: what ⌘↵ runs, the automatic LIMIT and confirmations. */
export function ExecutionPane(): React.JSX.Element {
  const settings = executionSettings.useValue();
  const [editor, updateEditor] = useEditorSettings();
  return (
    <section aria-label={t('execution')}>
      <h3>{t('runningQueries')}</h3>
      <div className={styles.settingsGroup}>
        <SettingRow title={t('runs')} description={t('whatRunButton')}>
          <SelectField
            aria-label={t('runs')}
            value={editor.runDefault}
            options={[
              { value: 'current', label: t('currentStatement') },
              { value: 'all', label: t('allStatements') },
            ]}
            onValueChange={(value) => {
              updateEditor({ runDefault: value === 'all' ? 'all' : 'current' });
            }}
          />
        </SettingRow>
        <SettingRow
          title={t('limitSelectResults')}
          description={t('appendLimitSelect')}
        >
          <SelectField
            aria-label={t('limitSelectResults')}
            value={JSON.stringify(settings.autoLimit)}
            options={AUTO_LIMITS.map((limit) => ({
              value: JSON.stringify(limit),
              label: limitLabel(limit),
            }))}
            onValueChange={(value) => {
              updateExecution({
                autoLimit: JSON.parse(value) as ExecutionSettings['autoLimit'],
              });
            }}
          />
        </SettingRow>
        {FLAGS.map((flag) => (
          <SettingRow
            key={flag.key}
            title={flag.title}
            description={flag.description}
          >
            <Switch
              aria-label={flag.title}
              checked={settings[flag.key]}
              onCheckedChange={(checked) => {
                updateExecution({ [flag.key]: checked });
              }}
            />
          </SettingRow>
        ))}
        <SettingRow
          title={t('returnError')}
          description={t('stopMultiStatement')}
        >
          <Switch
            aria-label={t('returnError')}
            checked={editor.returnOnError}
            onCheckedChange={(checked) => {
              updateEditor({ returnOnError: checked });
            }}
          />
        </SettingRow>
      </div>
    </section>
  );
}
