// SPDX-License-Identifier: Apache-2.0
import { SelectField, Switch } from '@openrows/ui';
import { SettingRow } from '../appearance/InterfaceSettings';
import { useEditorSettings } from '../query/editorSettings';
import type { EditorSettings } from '../query/editorSettings';
import {
  CHOICES,
  COMPLETION_FLAGS,
  DELAY_CHOICE,
  DENSE,
  FLAGS,
  FORMAT_CHOICES,
} from './editorPaneSpecs';
import type { Choice, FlagSpec } from './editorPaneSpecs';
import styles from '../appearance/Appearance.module.css';

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
      <h3>Completion</h3>
      <div className={styles.settingsGroup}>
        {COMPLETION_FLAGS.map((flag) => (
          <FlagRow key={flag.key} flag={flag} settings={settings} />
        ))}
        <ChoiceRow choice={DELAY_CHOICE} settings={settings} />
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
