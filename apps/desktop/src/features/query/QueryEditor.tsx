// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';
import type {
  CompletionCatalog,
  EditorDialect,
  EditorPreferences,
  EditorView,
  SqlEditor,
} from '@openrows/editor';
import { snippetSettings, snippetsFor } from '../snippets/snippetStore';
import { currentCatalog } from './completionCatalog';
import type { CursorStore } from './cursor';
import type { EditorSettings, KeyBinding } from './editorSettings';
import type { EditorHandle } from './request';
import styles from './Query.module.css';

/** Editor text reaches the tab descriptor at most every 300 ms and on unmount. */
const SAVE_DELAY_MS = 300;

type Props = {
  /** Initial text; later edits stay in the editor until saved through `onChange`. */
  doc: string;
  label: string;
  /** Highlighting dialect of the tab's connection. */
  dialect: EditorDialect;
  /** Bottom-bar settings, applied live without remounting. */
  settings: EditorSettings;
  /** Schema data for completion; `null` while disconnected. */
  catalog: CompletionCatalog | null;
  /** Receives the cursor position on every move. */
  cursor: CursorStore;
  onChange: (sql: string) => void;
  /** App shortcuts (run, format, files); true when the key was handled. */
  onKey: (event: KeyboardEvent) => boolean;
  /** Called with the editor once mounted, and with `null` when it goes. */
  onReady: (editor: EditorHandle | null) => void;
};

/** The editor's share of the settings (display and completion). */
function preferencesOf(settings: EditorSettings): EditorPreferences {
  const { fontSize, fontFamily, lineNumbers, folding } = settings;
  const { invisibles, wrap, highlightQuery } = settings;
  const { autocomplete, completionDelay, upperKeywords } = settings;
  const { completeSchemas, completeTables, completeColumns } = settings;
  return {
    fontSize,
    fontFamily,
    lineNumbers,
    folding,
    invisibles,
    wrap,
    highlightQuery,
    autocomplete,
    completionDelay,
    upperKeywords,
    completeSchemas,
    completeTables,
    completeColumns,
    completeSnippets: settings.completeSnippets,
    qualifyNames: settings.qualifyNames,
    closeDelimiters: settings.closeDelimiters,
    acceptKey: settings.acceptKey,
    indentWidth: settings.indentWidth,
    indentWith: settings.indentWith,
  };
}

function handleOf(editor: SqlEditor): EditorHandle {
  const { view } = editor;
  return {
    text: () => view.state.doc.toString(),
    cursor: () => view.state.selection.main.head,
    selection: () => {
      const { from, to } = view.state.selection.main;
      return view.state.sliceDoc(from, to);
    },
    focus: () => {
      view.focus();
    },
    format: (kind, options) => editor.format(kind, options),
    replace: (text) => {
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
    },
    insertSnippet: editor.insertSnippet,
    insertText: editor.insertText,
    runStart: editor.runStart,
  };
}

function reportCursor(view: EditorView, cursor: CursorStore): void {
  const location = view.state.selection.main.head;
  const line = view.state.doc.lineAt(location);
  cursor.set({
    line: line.number,
    column: location - line.from + 1,
    location,
  });
}

/** Install Vim (loaded on first use) or the default keys. */
function applyKeyBinding(
  editor: SqlEditor,
  binding: KeyBinding,
  wanted: () => KeyBinding,
): void {
  if (binding === 'default') {
    editor.setKeyMode(null);
    return;
  }
  void import('@openrows/editor/vim').then(({ vimMode }) => {
    if (wanted() === 'vim') editor.setKeyMode(vimMode());
  });
}

/** Hand a freshly mounted editor to the tab: key mode, handle, cursor, focus. */
function attach(editor: SqlEditor, latest: RefObject<Props>): void {
  const props = latest.current;
  const wanted = (): KeyBinding => latest.current.settings.keyBinding;
  if (props.settings.keyBinding !== 'default')
    applyKeyBinding(editor, props.settings.keyBinding, wanted);
  props.onReady(handleOf(editor));
  reportCursor(editor.view, props.cursor);
  editor.view.focus();
}

/** Mount CodeMirror (loaded lazily) and return the cleanup. */
function mount(
  host: RefObject<HTMLDivElement | null>,
  latest: RefObject<Props>,
  mounted: RefObject<SqlEditor | null>,
): () => void {
  let timer: number | undefined;
  let cancelled = false;
  const save = (): void => {
    window.clearTimeout(timer);
    timer = undefined;
    const view = mounted.current?.view;
    if (view) latest.current.onChange(view.state.doc.toString());
  };
  void import('@openrows/editor').then(({ mountSqlEditor }) => {
    const parent = host.current;
    if (cancelled || !parent) return;
    const props = latest.current;
    const editor = mountSqlEditor(parent, {
      doc: props.doc,
      label: props.label,
      dialect: props.dialect,
      catalog: currentCatalog(() => latest.current.catalog),
      snippets: () =>
        snippetsFor(snippetSettings.get(), latest.current.dialect),
      preferences: preferencesOf(props.settings),
      onKeyDown: (event) => latest.current.onKey(event),
      onDocChange: () => {
        window.clearTimeout(timer);
        timer = window.setTimeout(save, SAVE_DELAY_MS);
      },
      onSelection: (view) => {
        reportCursor(view, latest.current.cursor);
      },
    });
    mounted.current = editor;
    attach(editor, latest);
  });
  return () => {
    cancelled = true;
    if (timer !== undefined) save();
    latest.current.onReady(null);
    mounted.current?.destroy();
    mounted.current = null;
  };
}

/** CodeMirror loads lazily on first mount; one editor per tab (keyed by tab id). */
export function QueryEditor(props: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const mounted = useRef<SqlEditor | null>(null);
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  useEffect(() => mount(host, latest, mounted), []);
  const { settings } = props;
  useEffect(() => {
    mounted.current?.setPreferences(preferencesOf(settings));
  }, [settings]);
  const { keyBinding } = settings;
  useEffect(() => {
    const editor = mounted.current;
    if (editor)
      applyKeyBinding(
        editor,
        keyBinding,
        () => latest.current.settings.keyBinding,
      );
  }, [keyBinding]);
  return <div ref={host} className={styles.editor} />;
}
