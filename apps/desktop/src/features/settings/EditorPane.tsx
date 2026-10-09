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
  | 'autoSave'
  | 'denseOperators';

type FlagSpec = { key: Flag; title: string; description: string };

const FLAGS: readonly FlagSpec[] = [
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
  key:
    | 'fontSize'
    | 'keyBinding'
    | 'rowLimit'
    | 'keywordCase'
    | 'indentWidth'
    | 'indentWith'
    | 'indentStyle'
    | 'linesBetweenQueries';
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
];

/** Beautify (⌘I) and Uglify (⇧⌘I) layout. */
const FORMAT_CHOICES: readonly Choice[] = [
  {
    key: 'keywordCase',
    title: 'Keyword case',
    description: 'Case of SQL keywords when beautifying.',
    options: [
      { value: '"upper"', label: 'UPPER' },
      { value: '"lower"', label: 'lower' },
      { value: '"preserve"', label: 'As typed' },
    ],
  },
  {
    key: 'indentWidth',
    title: 'Indent width',
    description: 'Spaces per indent level.',
    options: [
      { value: '2', label: '2 spaces' },
      { value: '4', label: '4 spaces' },
    ],
  },
  {
    key: 'indentWith',
    title: 'Indent with',
    description: 'Indent with spaces or tabs.',
    options: [
      { value: '"spaces"', label: 'Spaces' },
      { value: '"tabs"', label: 'Tabs' },
    ],
  },
  {
    key: 'indentStyle',
    title: 'Indent type',
    description: 'Standard indents, or keywords aligned in a column.',
    options: [
      { value: '"standard"', label: 'Standard' },
      { value: '"tabularLeft"', label: 'Tabular left' },
      { value: '"tabularRight"', label: 'Tabular right' },
    ],
  },
  {
    key: 'linesBetweenQueries',
    title: 'Lines between queries',
    description: 'Empty lines between statements.',
    options: [
      { value: '1', label: '1' },
      { value: '2', label: '2' },
      { value: '3', label: '3' },
    ],
  },
];

/** Settings ▸ Editor: the SQL editor options of the bottom-bar menus. */
export function EditorPane(): React.JSX.Element {
  const [settings] = useEditorSettings();
  return (
    <section aria-label="Editor">
      <h3>SQL editor</h3>
      <div className={styles.settingsGroup}>
        {CHOICES.map((choice) => (
          <ChoiceRow key={choice.key} choice={choice} settings={settings} />
        ))}
        {FLAGS.map((flag) => (
          <FlagRow key={flag.key} flag={flag} settings={settings} />
        ))}
      </div>
      <h3>Formatter</h3>
      <div className={styles.settingsGroup}>
        {FORMAT_CHOICES.map((choice) => (
          <ChoiceRow key={choice.key} choice={choice} settings={settings} />
        ))}
        <FlagRow flag={DENSE} settings={settings} />
      </div>
    </section>
  );
}

const DENSE: FlagSpec = {
  key: 'denseOperators',
  title: 'Dense operators',
  description: 'Write operators without surrounding spaces (a=1).',
};

function ChoiceRow(props: {
  choice: Choice;
  settings: EditorSettings;
}): React.JSX.Element {
  const [, update] = useEditorSettings();
  const { key, title, description, options } = props.choice;
  return (
    <SettingRow title={title} description={description}>
      <SelectField
        aria-label={title}
        value={JSON.stringify(props.settings[key])}
        options={options}
        onValueChange={(value) => {
          update(Object.fromEntries([[key, JSON.parse(value) as unknown]]));
        }}
      />
    </SettingRow>
  );
}

function FlagRow(props: {
  flag: FlagSpec;
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
