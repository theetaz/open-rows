// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, Kbd } from '@openrows/ui';
import type { FormatKind } from '@openrows/editor';
import {
  beautifyEntries,
  limitEntries,
  limitLabel,
  runEntries,
  settingsEntries,
} from './barMenus';
import { cursorText, useCursor } from './cursor';
import type { CursorStore } from './cursor';
import type { EditorSettings } from './editorSettings';
import type { RunScope } from './request';
import { MenuButton, SplitButton } from './SplitButton';
import styles from './EditorBar.module.css';

type Props = {
  settings: EditorSettings;
  onSettings: (change: Partial<EditorSettings>) => void;
  cursor: CursorStore;
  /** A statement is running; Run becomes Cancel. */
  running: boolean;
  /** A session is open and nothing runs. */
  ready: boolean;
  /** The running query can be cancelled (its id is known). */
  cancellable: boolean;
  /** Why the tab cannot run, or a formatting error. */
  notice: string | null;
  /** No session: offer to connect instead. */
  offline: boolean;
  onConnect: () => void;
  onRun: (scope: RunScope) => void;
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
  if (props.running)
    return (
      <SplitButton
        variant="destructive"
        label="Cancel query"
        title="Cancel the running query"
        disabled={!props.cancellable}
        onClick={props.onCancel}
        menuLabel="Run options"
        entries={runEntries(props.onRun)}
        menuDisabled
      >
        Cancel
      </SplitButton>
    );
  return (
    <SplitButton
      variant="primary"
      label="Run current statement"
      title="Run current statement or selection (⌘↵)"
      disabled={!props.ready}
      onClick={() => {
        props.onRun('current');
      }}
      menuLabel="Run options"
      entries={runEntries(props.onRun)}
      menuDisabled={!props.ready}
    >
      Run Current
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
        entries={settingsEntries(settings, onSettings)}
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
        title="Beautify the selection or current statement (⌘I)"
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
