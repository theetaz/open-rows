// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Button, Icon } from '@openrows/ui';
import type { TableDetailDto } from '@openrows/ipc';
import { formatRows } from './format';
import { SqlViewer } from './SqlViewer';
import styles from './Structure.module.css';

/** Where the popover sits: above and centred on its footer button. */
function placement(anchor: DOMRect): React.CSSProperties {
  const width = Math.min(820, innerWidth - 32);
  const left = Math.max(
    16,
    Math.min(
      anchor.left + anchor.width / 2 - width / 2,
      innerWidth - width - 16,
    ),
  );
  return {
    left,
    width,
    bottom: innerHeight - anchor.top + 8,
    height: Math.max(160, Math.min(560, anchor.top - 56)),
  };
}

/** The rebuilt table definition above the DDL button (TablePlus' DDL popover). */
export function DdlPopover({
  detail,
  anchor,
  onClose,
}: {
  detail: TableDetailDto;
  anchor: DOMRect;
  onClose: () => void;
}): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState<string | null>(null);
  useEffect(() => {
    const outside = (event: Event): void => {
      const target = event.target;
      if (!(target instanceof Element)) return;
      if (ref.current?.contains(target) || target.closest('[data-ddl-button]'))
        return;
      onClose();
    };
    const escape = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('keydown', escape);
    };
  }, [onClose]);
  return createPortal(
    <div
      ref={ref}
      role="dialog"
      aria-label="Table definition"
      className={styles.popover}
      style={placement(anchor)}
    >
      <SqlViewer sql={detail.ddl} label="Table definition SQL" />
      <footer className={styles.popoverFooter}>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            navigator.clipboard.writeText(detail.ddl).then(
              () => {
                setCopied('Copied');
              },
              () => {
                setCopied('Clipboard access was blocked');
              },
            );
          }}
        >
          <Icon name="copy" />
          {copied ?? 'Copy'}
        </Button>
        <span>Estimated rows {formatRows(detail.info.rowEstimate)}</span>
      </footer>
    </div>,
    document.body,
  );
}
