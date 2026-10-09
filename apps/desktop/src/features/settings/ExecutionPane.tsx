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

type Flag = 'runSelection' | 'confirmMultiple' | 'showNotices';

const FLAGS: readonly { key: Flag; title: string; description: string }[] = [
  {
    key: 'runSelection',
    title: 'Run selection',
    description: '⌘↵ with text selected runs exactly the selection.',
  },
  {
    key: 'confirmMultiple',
    title: 'Confirm multiple statements',
    description: 'Ask before a run of more than one statement starts.',
  },
  {
    key: 'showNotices',
    title: 'Show server notices',
    description: 'List notices and warnings from the server with the results.',
  },
];

const limitLabel = (limit: ExecutionSettings['autoLimit']): string =>
  limit === null ? 'Off' : `LIMIT ${limit.toLocaleString('en-US')}`;

/** Settings ▸ Execution: what ⌘↵ runs, the automatic LIMIT and confirmations. */
export function ExecutionPane(): React.JSX.Element {
  const settings = executionSettings.use();
  const [editor, updateEditor] = useEditorSettings();
  return (
    <section aria-label="Execution">
      <h3>Running queries</h3>
      <div className={styles.settingsGroup}>
        <SettingRow
          title="⌘↵ runs"
          description="What the Run button and ⌘↵ run without a selection; ⇧⌘↵ runs the other."
        >
          <SelectField
            aria-label="⌘↵ runs"
            value={editor.runDefault}
            options={[
              { value: 'current', label: 'Current statement' },
              { value: 'all', label: 'All statements' },
            ]}
            onValueChange={(value) => {
              updateEditor({ runDefault: value === 'all' ? 'all' : 'current' });
            }}
          />
        </SettingRow>
        <SettingRow
          title="Limit SELECT results"
          description="Append LIMIT to SELECT statements without one; Run Without Limit skips it once."
        >
          <SelectField
            aria-label="Limit SELECT results"
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
          title="Return on error"
          description="Stop a multi-statement run at the first error."
        >
          <Switch
            aria-label="Return on error"
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
