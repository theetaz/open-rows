// SPDX-License-Identifier: Apache-2.0
import { Button, Icon, Kbd } from '@openrows/ui';
import type { EditorDialect, FormatKind } from '@openrows/editor';
import { limitEntries, limitLabel, moreEntries } from './barMenus';
import { cursorText, useCursor } from './cursor';
import type { CursorStore } from './cursor';
import type { EditorSettings } from './editorSettings';
import { runEntries, runLabel } from './runMenu';
import type { RunMenu } from './runMenu';
import { MenuButton, SplitButton } from './SplitButton';
import styles from './EditorBar.module.css';
import { t } from './lazyMessages';

type Props = {
  settings: EditorSettings;
  onSettings: (change: Partial<EditorSettings>) => void;
  /** Engine of the tab, for the parameter styles it offers. */
  dialect: EditorDialect;
  /** A statement is running; Run becomes Cancel. */
  running: boolean;
  /** The running query can be cancelled (its id is known). */
  cancellable: boolean;
  /** Why the tab cannot run, or a formatting error. */
  notice: string | null;
  /** Run's dropdown; `run.ready` also enables the Run button. */
  run: RunMenu;
  /** The Run button: the selection, else the default scope. */
  onRunDefault: () => void;
  onCancel: () => void;
  onFormat: (kind: FormatKind) => void;
  /** Explain (`analyze`: Explain Analyze) the statement at the cursor or the selection. */
  onExplain?: ((analyze: boolean) => void) | undefined;
  /** Opens the AI assistant, which writes or explains queries from plain language. */
  onAssistant?: (() => void) | undefined;
  /** Opens the ⌘I prompt over the selection or current statement. */
  onInline?: (() => void) | undefined;
};

/** Run (or Cancel while running), the bar's only split button. */
function RunButton(props: Props): React.JSX.Element {
  const all = props.run.runDefault === 'all';
  const entries = runEntries(props.run, props.onExplain);
  if (props.running)
    return (
      <SplitButton
        variant="destructive"
        label={t('cancelQuery')}
        title={t('cancelRunningQuery')}
        disabled={!props.cancellable}
        onClick={props.onCancel}
        menuLabel={t('runOptions')}
        entries={entries}
      >
        {t('cancel')}
      </SplitButton>
    );
  return (
    <SplitButton
      variant="primary"
      label={all ? t('runAllStatements') : t('runCurrentStatement')}
      title={
        all ? t('runAllStatementsSelection') : t('runCurrentStatementSelection')
      }
      disabled={!props.run.ready}
      onClick={props.onRunDefault}
      menuLabel={t('runOptions')}
      entries={entries}
    >
      <Icon name="play" />
      {runLabel(props.run.runDefault)}
      <Kbd variant="inline" aria-hidden="true">
        ⌘↵
      </Kbd>
    </SplitButton>
  );
}

/** Beautify, Explain and AI: plain buttons; their other commands live in the Run and More menus. */
function ActionButtons(props: Props): React.JSX.Element {
  const { onExplain, onAssistant } = props;
  return (
    <>
      <Button
        variant="secondary"
        title={t('beautifySelectionWhole')}
        onClick={() => {
          props.onFormat('beautify');
        }}
      >
        <Icon name="wand" />
        {t('beautify')}
      </Button>
      {onExplain && (
        <Button
          variant="secondary"
          title={t('explainStatementCursor')}
          disabled={!props.run.ready}
          onClick={() => {
            onExplain(false);
          }}
        >
          <Icon name="activity" />
          {t('explain')}
        </Button>
      )}
      {onAssistant && (
        <Button
          variant="secondary"
          aria-label={t('ai')}
          title={t('openAiAssistant')}
          onClick={onAssistant}
        >
          <Icon name="sparkles" />
          {t('ai')}
        </Button>
      )}
    </>
  );
}

/**
 * The editor's bar between the SQL and its results (the prototype's
 * `.qbar`): Run, Beautify, Explain and AI on the left, then the notice, the
 * row limit and More, all 28 px capsules.
 */
export function EditorBar(props: Props): React.JSX.Element {
  const { settings, onSettings } = props;
  return (
    <div className={styles.bar} role="toolbar" aria-label={t('queryActions')}>
      <RunButton {...props} />
      <ActionButtons {...props} />
      <span className={styles.notice}>{props.notice}</span>
      <MenuButton
        label={t('rowLimit')}
        title={t('rowsKeptPer')}
        className={styles.limit}
        align="end"
        entries={limitEntries(settings, onSettings)}
      >
        {limitLabel(settings.rowLimit)}
        <Icon name="chevronUpDown" />
      </MenuButton>
      <MenuButton
        label={t('moreQueryActions')}
        title={t('more')}
        variant="ghost"
        className={styles.icon}
        align="end"
        entries={moreEntries({
          settings,
          update: onSettings,
          dialect: props.dialect,
          format: props.onFormat,
          inline: props.onInline,
        })}
      >
        <Icon name="more" />
      </MenuButton>
    </div>
  );
}

/** The cursor's line and column, for the tab's status line. */
export function CursorText({
  store,
}: {
  store: CursorStore;
}): React.JSX.Element {
  return (
    <span className={styles.cursor} aria-label={t('cursorPosition')}>
      {cursorText(useCursor(store))}
    </span>
  );
}
