// SPDX-License-Identifier: Apache-2.0
import { useEffect, useLayoutEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { Button } from '@openrows/ui';
import { reformat } from './cellKind';
import { editorKey } from './editorKey';
import type { InlineEditor, OpenEditor } from './useInlineEditor';
import styles from './Inline.module.css';

type Props = { open: OpenEditor; editor: InlineEditor };

/** Long, multi-line or JSON values: a wrapping monospace editor anchored to the cell. */
export function ValuePopover({ open, editor }: Props): React.JSX.Element {
  const panel = useRef<HTMLDivElement>(null);
  const text = useRef<HTMLTextAreaElement>(null);
  const top = open.anchor.y - open.rect.height;
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const below = innerHeight - element.offsetHeight - 8;
    element.style.left = `${String(Math.max(8, Math.min(open.anchor.x, innerWidth - element.offsetWidth - 8)))}px`;
    element.style.top = `${String(Math.max(8, Math.min(top, below)))}px`;
  }, [open.anchor.x, top]);
  useEffect(() => {
    text.current?.focus({ preventScroll: true });
  }, []);
  return createPortal(
    <div
      ref={panel}
      role="dialog"
      aria-label={`Edit ${open.edit.name}`}
      className={styles.popover}
    >
      <textarea
        ref={text}
        className={styles.popoverText}
        aria-label={`Value of ${open.edit.name}`}
        spellCheck={false}
        readOnly={!open.ready}
        value={open.value}
        onChange={(event) => {
          editor.change(event.target.value);
        }}
        onKeyDown={(event) => {
          editorKey(event, { open, editor }, true);
        }}
      />
      <PopoverBar open={open} editor={editor} />
    </div>,
    document.body,
  );
}

/** JSON reformatting, copy, cancel and save (⌘↩). */
function PopoverBar({ open, editor }: Props): React.JSX.Element {
  const json = /json/i.test(open.edit.dataType);
  return (
    <div className={styles.popoverBar}>
      {json && (
        <>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              editor.change(reformat(open.value, true));
            }}
          >
            Pretty
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              editor.change(reformat(open.value, false));
            }}
          >
            Minify
          </Button>
        </>
      )}
      <Button
        size="sm"
        variant="ghost"
        onClick={() => {
          void navigator.clipboard.writeText(open.value);
        }}
      >
        Copy
      </Button>
      <span className={styles.spacer} />
      <Button size="sm" onClick={editor.cancel}>
        Cancel
      </Button>
      <Button
        size="sm"
        variant="primary"
        title="Stage (⌘↩)"
        disabled={!open.ready}
        onClick={() => {
          editor.commit(open.value);
        }}
      >
        Save
      </Button>
    </div>
  );
}
