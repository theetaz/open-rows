// SPDX-License-Identifier: Apache-2.0
/** Shared arrow/Home/End navigation and Escape dismissal for action menus. */
export function menuKey(event: React.KeyboardEvent, close: () => void): void {
  if (event.nativeEvent.isComposing) return;
  if (event.metaKey) {
    close();
    return;
  }
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault();
    event.stopPropagation();
    close();
    return;
  }
  const items = Array.from(
    event.currentTarget.querySelectorAll<HTMLButtonElement>(
      '[role^="menuitem"]:not(:disabled)',
    ),
  );
  const index = items.indexOf(document.activeElement as HTMLButtonElement);
  const next = menuIndex(event.key, index, items.length);
  if (next === undefined) return;
  event.preventDefault();
  items[next]?.focus();
}
function menuIndex(
  key: string,
  index: number,
  length: number,
): number | undefined {
  if (key === 'Home') return 0;
  if (key === 'End') return length - 1;
  if (key === 'ArrowDown') return (index + 1) % length;
  if (key === 'ArrowUp') return (index - 1 + length) % length;
  return undefined;
}
