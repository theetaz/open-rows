// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import type { EditorDialect, EditorView, SqlEditor } from '@openrows/editor';
import type { EditorHandle, RunScope } from './request';
import styles from './Query.module.css';

/** Editor text reaches the tab descriptor at most every 300 ms and on unmount. */
const SAVE_DELAY_MS = 300;

type Props = {
  /** Initial text; later edits stay in the editor until saved through `onChange`. */
  doc: string;
  label: string;
  /** Highlighting dialect of the tab's connection. */
  dialect: EditorDialect;
  onChange: (sql: string) => void;
  onRun: (scope: RunScope) => void;
  /** Called with the editor once mounted, and with `null` when it goes. */
  onReady: (editor: EditorHandle | null) => void;
};

function handleOf(view: EditorView): EditorHandle {
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
  };
}

/** CodeMirror loads lazily on first mount; one editor per tab (keyed by tab id). */
export function QueryEditor(props: Props): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const latest = useRef(props);
  useEffect(() => {
    latest.current = props;
  });
  useEffect(() => {
    let editor: SqlEditor | undefined;
    let timer: number | undefined;
    let cancelled = false;
    const save = (): void => {
      window.clearTimeout(timer);
      timer = undefined;
      if (editor) latest.current.onChange(editor.view.state.doc.toString());
    };
    const run = (scope: RunScope) => (): boolean => {
      latest.current.onRun(scope);
      return true;
    };
    void import('@openrows/editor').then(({ mountSqlEditor }) => {
      const parent = host.current;
      if (cancelled || !parent) return;
      editor = mountSqlEditor(parent, {
        doc: latest.current.doc,
        label: latest.current.label,
        dialect: latest.current.dialect,
        keys: [
          { key: 'Mod-Enter', run: run('current') },
          { key: 'Shift-Mod-Enter', run: run('all') },
        ],
        onDocChange: () => {
          window.clearTimeout(timer);
          timer = window.setTimeout(save, SAVE_DELAY_MS);
        },
      });
      latest.current.onReady(handleOf(editor.view));
      editor.view.focus();
    });
    return () => {
      cancelled = true;
      if (timer !== undefined) save();
      latest.current.onReady(null);
      editor?.destroy();
    };
  }, []);
  return <div ref={host} className={styles.editor} />;
}
