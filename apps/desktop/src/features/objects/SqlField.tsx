// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import type { SqlEditor } from '@openrows/editor';
import type { EngineDto } from '@openrows/ipc';
import styles from '../table/structure/forms/Forms.module.css';

import { editorDialect } from '../connections/engines';

/**
 * An editable SQL definition in the app's SQL editor (lazily loaded, with
 * highlighting for the engine); a plain text area until it has mounted.
 * `value` seeds the editor; edits come back through `onChange`.
 */
export function SqlField({
  value,
  label,
  engine,
  onChange,
}: {
  value: string;
  label: string;
  engine: EngineDto;
  onChange: (sql: string) => void;
}): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  const [loaded, setLoaded] = useState(false);
  const seed = useRef(value);
  const changed = useRef(onChange);
  useEffect(() => {
    changed.current = onChange;
  });
  useEffect(() => {
    let cancelled = false;
    let editor: SqlEditor | undefined;
    void import('@openrows/editor').then(({ mountSqlEditor }) => {
      const parent = host.current;
      if (cancelled || !parent) return;
      editor = mountSqlEditor(parent, {
        doc: seed.current,
        label,
        dialect: editorDialect(engine),
        onDocChange: () => {
          if (editor) changed.current(editor.view.state.doc.toString());
        },
      });
      setLoaded(true);
    });
    return () => {
      cancelled = true;
      editor?.destroy();
    };
  }, [label, engine]);
  return (
    <div className={styles.editor} data-loaded={loaded}>
      <div ref={host} />
      {!loaded && (
        <textarea
          aria-label={label}
          value={value}
          spellCheck={false}
          onChange={(event) => {
            seed.current = event.target.value;
            onChange(event.target.value);
          }}
        />
      )}
    </div>
  );
}
