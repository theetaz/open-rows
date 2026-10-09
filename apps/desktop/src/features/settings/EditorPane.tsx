// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import {
  FONT_SIZES,
  ROW_LIMITS,
  useEditorSettings,
} from '../query/editorSettings';
import type { EditorSettings } from '../query/editorSettings';
import styles from '../appearance/Appearance.module.css';

type Flag =
  | 'wrap'
  | 'invisibles'
  | 'highlightQuery'
  | 'autocomplete'
  | 'upperKeywords'
  | 'autoSave';

const FLAGS: readonly { key: Flag; title: string; description: string }[] = [
  { key: 'wrap', title: 'Wrap lines', description: 'Soft-wrap long lines.' },
  {
    key: 'invisibles',
    title: 'Show invisibles',
    description: 'Mark spaces and tabs.',
  },
  {
    key: 'highlightQuery',
    title: 'Highlight current query',
    description: 'Tint the statement ⌘↵ runs.',
  },
  {
    key: 'autocomplete',
    title: 'Autocomplete',
    description: 'Suggest keywords, tables and columns while typing.',
  },
  {
    key: 'upperKeywords',
    title: 'Uppercase keywords',
    description: 'Complete keywords in upper case.',
  },
  {
    key: 'autoSave',
    title: 'Auto Save',
    description:
      'Keep query text across launches and write file-bound tabs as you type.',
  },
];

type Choice = {
  key: 'fontSize' | 'keyBinding' | 'rowLimit' | 'keywordCase';
  title: string;
  description: string;
  options: readonly { value: string; label: string }[];
};

const CHOICES: readonly Choice[] = [
  {
    key: 'fontSize',
    title: 'Font size',
    description: 'Text size of SQL editors.',
    options: [
      { value: 'null', label: 'Appearance default' },
      ...FONT_SIZES.map((size) => ({
        value: String(size),
        label: `${String(size)} px`,
      })),
    ],
  },
  {
    key: 'keyBinding',
    title: 'Key bindings',
    description: 'Editing keys of SQL editors.',
    options: [
      { value: '"default"', label: 'Default' },
      { value: '"vim"', label: 'Vim' },
    ],
  },
  {
    key: 'rowLimit',
    title: 'Row limit',
    description: 'Rows kept per result set when running.',
    options: [
      { value: 'null', label: 'No limit' },
      ...ROW_LIMITS.map((rows) => ({
        value: String(rows),
        label: rows.toLocaleString('en-US'),
      })),
    ],
  },
  {
    key: 'keywordCase',
    title: 'Beautify keywords',
    description: 'Keyword case of Beautify (⌘I).',
    options: [
      { value: '"upper"', label: 'UPPER' },
      { value: '"lower"', label: 'lower' },
      { value: '"preserve"', label: 'As typed' },
    ],
  },
];

/** Settings ▸ Editor: the SQL editor options of the bottom-bar menus. */
export function EditorPane(): React.JSX.Element {
  const [settings, update] = useEditorSettings();
  return (
    <section aria-label="Editor">
      <h3>SQL editor</h3>
      <div className={styles.settingsGroup}>
        {CHOICES.map((choice) => (
          <SettingRow
            key={choice.key}
            title={choice.title}
            description={choice.description}
          >
            <SelectField
              aria-label={choice.title}
              value={JSON.stringify(settings[choice.key])}
              options={choice.options}
              onValueChange={(value) => {
                update(
                  Object.fromEntries([
                    [choice.key, JSON.parse(value) as unknown],
                  ]),
                );
              }}
            />
          </SettingRow>
        ))}
        {FLAGS.map((flag) => (
          <FlagRow key={flag.key} flag={flag} settings={settings} />
        ))}
      </div>
    </section>
  );
}

function FlagRow(props: {
  flag: (typeof FLAGS)[number];
  settings: EditorSettings;
}): React.JSX.Element {
  const [, update] = useEditorSettings();
  const { key, title, description } = props.flag;
  return (
    <SettingRow title={title} description={description}>
      <Switch
        aria-label={title}
        checked={props.settings[key]}
        onCheckedChange={(checked) => {
          update({ [key]: checked });
        }}
      />
    </SettingRow>
  );
}
