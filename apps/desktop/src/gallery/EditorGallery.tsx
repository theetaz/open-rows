// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { Switch } from '@openrows/ui';
import type { SqlEditor } from '@openrows/editor';
import styles from './Gallery.module.css';

const SPECIMEN = `-- Type here to try highlighting, completion, Vim and IME input.
-- 日本語や中文のコメントも入力できます。
WITH recent AS (
  SELECT o.customer_id, o.total, o.payload ->> 'channel' AS channel
  FROM sales.orders AS o
  WHERE o.created_at > now() - interval '7 days'
)
SELECT channel, sum(total)::numeric(12, 2) AS revenue
FROM recent
GROUP BY channel
ORDER BY revenue DESC;`;

/** Live CodeMirror specimen; editor and Vim code load only when this section mounts. */
export function EditorGallery(): React.JSX.Element {
  const [vim, setVim] = useState(false);
  return (
    <section
      id="editor"
      aria-labelledby="editor-heading"
      className={styles.section}
    >
      <h2 id="editor-heading">SQL editor</h2>
      <div className={styles.row}>
        <Switch aria-label="Vim mode" checked={vim} onCheckedChange={setVim} />
        <span>Vim mode {vim ? 'on' : 'off'}</span>
      </div>
      <EditorSpecimen key={vim ? 'vim' : 'standard'} vim={vim} />
    </section>
  );
}

function EditorSpecimen({ vim }: { vim: boolean }): React.JSX.Element {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let editor: SqlEditor | undefined;
    let cancelled = false;
    // Escape belongs to the editor (Vim, completion), not the gallery's close shortcut.
    const keepEscape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') event.stopPropagation();
    };
    element.addEventListener('keydown', keepEscape);
    void mountSpecimen(element, vim).then((mounted) => {
      if (cancelled) mounted.destroy();
      else editor = mounted;
    });
    return () => {
      cancelled = true;
      element.removeEventListener('keydown', keepEscape);
      editor?.destroy();
    };
  }, [vim]);
  return <div ref={host} className={styles.editor} />;
}

async function mountSpecimen(
  element: HTMLElement,
  vim: boolean,
): Promise<SqlEditor> {
  const { mountSqlEditor } = await import('@openrows/editor');
  const keyMode = vim
    ? (await import('@openrows/editor/vim')).vimMode()
    : undefined;
  return mountSqlEditor(element, {
    doc: SPECIMEN,
    label: 'SQL editor specimen',
    ...(keyMode ? { keyMode } : {}),
  });
}
