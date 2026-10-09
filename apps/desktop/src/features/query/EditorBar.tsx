// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, Kbd } from '@openrows/ui';
import type { EditorDialect, FormatKind } from '@openrows/editor';
import {
  beautifyEntries,
  limitEntries,
  limitLabel,
  settingsEntries,
} from './barMenus';
import { cursorText, useCursor } from './cursor';
import type { CursorStore } from './cursor';
import type { EditorSettings } from './editorSettings';
import { runEntries, runLabel } from './runMenu';
import type { RunMenu } from './runMenu';
import { MenuButton, SplitButton } from './SplitButton';
import styles from './EditorBar.module.css';

type Props = {
  settings: EditorSettings;
  onSettings: (change: Partial<EditorSettings>) => void;
  /** Engine of the tab, for the parameter styles it offers. */
  dialect: EditorDialect;
  cursor: CursorStore;
  /** A statement is running; Run becomes Cancel. */
  running: boolean;
  /** The running query can be cancelled (its id is known). */
  cancellable: boolean;
  /** Why the tab cannot run, or a formatting error. */
  notice: string | null;
  /** No session: offer to connect instead. */
  offline: boolean;
  onConnect: () => void;
  /** Run's dropdown; `run.ready` also enables the Run button. */
  run: RunMenu;
  /** The Run button: the selection, else the default scope. */
  onRunDefault: () => void;
  onCancel: () => void;
  onFormat: (kind: FormatKind) => void;
};

function CursorText({ store }: { store: CursorStore }): React.JSX.Element {
  return (
    <span className={styles.cursor} aria-label="Cursor position">
      {cursorText(useCursor(store))}
    </span>
  );
}

function RunButton(props: Props): React.JSX.Element {
  const all = props.run.runDefault === 'all';
  if (props.running)
    return (
      <SplitButton
        variant="destructive"
        label="Cancel query"
        title="Cancel the running query"
        disabled={!props.cancellable}
        onClick={props.onCancel}
        menuLabel="Run options"
        entries={runEntries(props.run)}
      >
        Cancel
      </SplitButton>
    );
  return (
    <SplitButton
      variant="primary"
      label={all ? 'Run all statements' : 'Run current statement'}
      title={`Run ${all ? 'all statements' : 'current statement'} or selection (⌘↵)`}
      disabled={!props.run.ready}
      onClick={props.onRunDefault}
      menuLabel="Run options"
      entries={runEntries(props.run)}
    >
      {runLabel(props.run.runDefault)}
      <Kbd variant="inline" aria-hidden="true">
        ⌘↵
      </Kbd>
    </SplitButton>
  );
}

/**
 * The bar between editor and results (TablePlus layout): editor settings,
 * cursor position, then row limit, Beautify and Run split buttons.
 */
export function EditorBar(props: Props): React.JSX.Element {
  const { settings, onSettings } = props;
  return (
    <div className={styles.bar} role="toolbar" aria-label="Query actions">
      <MenuButton
        label="Editor settings"
        variant="ghost"
        className={styles.icon}
        entries={settingsEntries(settings, onSettings, props.dialect)}
      >
        <Icon name="settings" />
      </MenuButton>
      <CursorText store={props.cursor} />
      <span className={styles.notice}>
        {props.notice ??
          (props.offline && (
            <Button size="sm" variant="ghost" onClick={props.onConnect}>
              Connect to run
            </Button>
          ))}
      </span>
      <MenuButton
        label="Row limit"
        title="Rows kept per result set"
        className={styles.limit}
        entries={limitEntries(settings, onSettings)}
      >
        {limitLabel(settings.rowLimit)}
        <Icon name="chevronDown" />
      </MenuButton>
      <SplitButton
        label="Beautify"
        title="Beautify the selection or the whole query (⌘I)"
        onClick={() => {
          props.onFormat('beautify');
        }}
        menuLabel="Format options"
        entries={beautifyEntries(settings, onSettings, props.onFormat)}
      >
        Beautify
      </SplitButton>
      <RunButton {...props} />
    </div>
  );
}
