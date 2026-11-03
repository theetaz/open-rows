// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { listWindows } from '@openrows/ipc';
import type { WindowEntryDto } from '@openrows/ipc';
import { t } from './messages';

/** A window's menu name: its number, then the title its workspace set. */
export function windowName(entry: WindowEntryDto): string {
  const number =
    entry.label === 'main' ? '1' : entry.label.replace('workspace-', '');
  const title = entry.title && entry.title !== 'OpenRows' ? entry.title : '';
  return title
    ? t('windowNumberTitle', { number, title })
    : t('windowNumber', { number });
}

/** The other open workspace windows, read when a menu opens. */
export function useOtherWindows(): readonly WindowEntryDto[] {
  const [windows, setWindows] = useState<readonly WindowEntryDto[]>([]);
  useEffect(() => {
    let live = true;
    listWindows().then(
      (list) => {
        if (live) setWindows(list.filter((entry) => !entry.current));
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);
  return windows;
}
