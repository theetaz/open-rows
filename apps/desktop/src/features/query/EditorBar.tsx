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
  /** No session: offer to connect instead. */
  offline: boolean;
  onConnect: () => void;
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

function RunButton(props: Props): React.JSX.Element {
  const all = props.run.runDefault === 'all';
  if (props.running)
    return (
      <SplitButton
        variant="destructive"
        label={t('cancelQuery')}
        title={t('cancelRunningQuery')}
        disabled={!props.cancellable}
        onClick={props.onCancel}
        menuLabel={t('runOptions')}
        entries={runEntries(props.run)}
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
      entries={runEntries(props.run)}
    >
      {runLabel(props.run.runDefault)}
      <Kbd variant="inline" aria-hidden="true">
        ⌘↵
      </Kbd>
    </SplitButton>
  );
}

/** Explain, with Explain Analyze in its menu. */
function ExplainButton(props: {
  ready: boolean;
  onExplain: (analyze: boolean) => void;
}): React.JSX.Element {
  const entry = (label: string, analyze: boolean, shortcut: string) => ({
    kind: 'item' as const,
    label,
    shortcut,
    disabled: !props.ready,
    run: () => {
      props.onExplain(analyze);
    },
  });
  return (
    <SplitButton
      label={t('explain')}
      title={t('explainStatementCursor')}
      disabled={!props.ready}
      onClick={() => {
        props.onExplain(false);
      }}
      menuLabel={t('explainOptions')}
      entries={[
        entry(t('explain'), false, '⌥⌘E'),
        entry(t('explainAnalyzeRuns'), true, '⌥⇧⌘E'),
      ]}
    >
      {t('explain')}
    </SplitButton>
  );
}

/** One AI control: the assistant panel, with ⌘I inline editing in its menu. */
function AiButton(props: {
  onAssistant: () => void;
  onInline?: (() => void) | undefined;
}): React.JSX.Element {
  const { onInline } = props;
  return (
    <SplitButton
      label={t('ai')}
      title={t('openAiAssistant')}
      className={styles.ai}
      onClick={props.onAssistant}
      menuLabel={t('aiOptions')}
      entries={[
        ...(onInline
          ? [
              {
                kind: 'item' as const,
                label: t('editSqlAi'),
                shortcut: '⌘I',
                run: onInline,
              },
            ]
          : []),
        { kind: 'item', label: t('assistantPanel'), run: props.onAssistant },
      ]}
    >
      <Icon name="sparkles" />
      {t('ai')}
    </SplitButton>
  );
}

/**
 * The editor's view bar above the SQL (the kit's `.viewbar`): Run, Explain,
 * Beautify and AI on the left, then the notice, the row limit and the
 * editor settings.
 */
export function EditorBar(props: Props): React.JSX.Element {
  const { settings, onSettings } = props;
  return (
    <div className={styles.bar} role="toolbar" aria-label={t('queryActions')}>
      <RunButton {...props} />
      {props.onExplain && (
        <ExplainButton ready={props.run.ready} onExplain={props.onExplain} />
      )}
      <SplitButton
        label={t('beautify')}
        title={t('beautifySelectionWhole')}
        onClick={() => {
          props.onFormat('beautify');
        }}
        menuLabel={t('formatOptions')}
        entries={beautifyEntries(settings, onSettings, props.onFormat)}
      >
        {t('beautify')}
      </SplitButton>
      {props.onAssistant && (
        <AiButton onAssistant={props.onAssistant} onInline={props.onInline} />
      )}
      <span className={styles.notice}>
        {props.notice ??
          (props.offline && (
            <Button size="sm" variant="ghost" onClick={props.onConnect}>
              {t('connectRun')}
            </Button>
          ))}
      </span>
      <span className={styles.limitLabel} aria-hidden="true">
        {t('rowLimit')}
      </span>
      <MenuButton
        label={t('rowLimit')}
        title={t('rowsKeptPer')}
        className={styles.limit}
        entries={limitEntries(settings, onSettings)}
      >
        {limitLabel(settings.rowLimit)}
        <Icon name="chevronUpDown" />
      </MenuButton>
      <MenuButton
        label={t('editorSettings')}
        variant="ghost"
        className={styles.icon}
        entries={settingsEntries(settings, onSettings, props.dialect)}
      >
        <Icon name="settings" />
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
