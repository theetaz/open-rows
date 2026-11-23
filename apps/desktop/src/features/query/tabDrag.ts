// SPDX-License-Identifier: Apache-2.0
import { material } from '@openrows/ui/tokens';

/** Pointer travel before a press on a tab becomes a drag. */
const THRESHOLD_PX = 4;
/** Room between two tabs (the strip's gap). */
const GAP_PX = 2;
const SETTLE = 'cubic-bezier(0.34, 1.42, 0.64, 1)';

/** Where a dragged tab lands: before `others[index]`, or after them all. */
export type TabDrop = { id: string; index: number; others: string[] };

type Drag = {
  node: HTMLElement;
  siblings: HTMLElement[];
  start: number;
  centres: number[];
  width: number;
  dx: number;
  target: number;
};

const ms = (token: string, reduced: boolean): number =>
  reduced ? 0 : parseFloat(token);

/** Tabs of the same kind (pinned or not) as `node`, in strip order. */
function siblingsOf(list: HTMLElement, node: HTMLElement): HTMLElement[] {
  return [...list.querySelectorAll<HTMLElement>('[data-tab-id]')].filter(
    (item) => item.dataset.pinned === node.dataset.pinned,
  );
}

/** Slot among the other tabs the dragged tab's centre has passed. */
function targetOf(drag: Drag): number {
  const centre = (drag.centres[drag.start] ?? 0) + drag.dx;
  return drag.centres.filter(
    (other, index) => index !== drag.start && other < centre,
  ).length;
}

/** Neighbours slide aside to show where the tab will land. */
function shiftNeighbours(drag: Drag, reduced: boolean): void {
  const step = drag.width + GAP_PX;
  drag.siblings.forEach((item, index) => {
    if (index === drag.start) return;
    const among = index < drag.start ? index : index - 1;
    let shift = 0;
    if (index > drag.start && among < drag.target) shift = -step;
    if (index < drag.start && among >= drag.target) shift = step;
    item.style.transition = `transform ${String(ms(material['motion-standard'], reduced))}ms ${material['motion-easing']}`;
    item.style.transform = shift ? `translateX(${String(shift)}px)` : '';
  });
}

/** Swallow the click that ends a drag, so dropping does not select. */
function swallowNextClick(): void {
  const swallow = (event: Event): void => {
    event.stopPropagation();
    event.preventDefault();
  };
  window.addEventListener('click', swallow, { capture: true, once: true });
  window.setTimeout(() => {
    window.removeEventListener('click', swallow, { capture: true });
  });
}

/** Every chip and tab of the strip with where it is drawn now. */
function snapshot(list: HTMLElement): Map<HTMLElement, number> {
  const boxes = new Map<HTMLElement, number>();
  for (const item of list.querySelectorAll<HTMLElement>(
    '[data-tab-id], [data-chip]',
  ))
    boxes.set(item, item.getBoundingClientRect().left);
  return boxes;
}

/**
 * After the model reordered: clear the drag offsets, then play each item
 * from where it was drawn to its new slot (FLIP); the tab itself settles
 * with the spring curve.
 */
function settle(
  drag: Drag,
  before: Map<HTMLElement, number>,
  reduced: boolean,
): void {
  for (const item of before.keys()) {
    item.style.transition = '';
    item.style.transform = '';
  }
  // Without Web Animations (tests), the tabs land in place at once.
  if (reduced || !('animate' in Element.prototype)) {
    delete drag.node.dataset.dragging;
    return;
  }
  for (const [item, left] of before) {
    const delta = left - item.getBoundingClientRect().left;
    const own = item === drag.node;
    if (!delta && !own) continue;
    const from = `translateX(${String(delta)}px)${own ? ' scale(1.02)' : ''}`;
    item.animate([{ transform: from }, { transform: 'none' }], {
      duration: ms(
        material[own ? 'motion-settle' : 'motion-standard'],
        reduced,
      ),
      easing: own ? SETTLE : material['motion-easing'],
    }).onfinish = () => {
      if (own) delete item.dataset.dragging;
    };
  }
}

/**
 * Track a primary-button press on a tab. Past 4 px the tab lifts and
 * follows the pointer while its neighbours slide aside; on release
 * `onDrop` reorders the model synchronously and everything settles.
 */
export function startTabDrag(
  event: React.PointerEvent<HTMLElement>,
  options: { reduced: boolean; onDrop: (drop: TabDrop) => void },
): void {
  const node = event.currentTarget;
  const list = node.parentElement;
  if (event.button !== 0 || !list) return;
  const siblings = siblingsOf(list, node);
  const drag: Drag = {
    node,
    siblings,
    start: siblings.indexOf(node),
    centres: siblings.map((item) => {
      const box = item.getBoundingClientRect();
      return box.left + box.width / 2;
    }),
    width: node.getBoundingClientRect().width,
    dx: 0,
    target: siblings.indexOf(node),
  };
  const startX = event.clientX;
  let dragging = false;
  const move = (next: PointerEvent): void => {
    if (!dragging && Math.abs(next.clientX - startX) < THRESHOLD_PX) return;
    dragging = true;
    node.dataset.dragging = 'true';
    drag.dx = next.clientX - startX;
    node.style.transition = 'none';
    node.style.transform = `translateX(${String(drag.dx)}px) scale(1.02)`;
    drag.target = targetOf(drag);
    shiftNeighbours(drag, options.reduced);
  };
  const end = (): void => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', end);
    window.removeEventListener('pointercancel', end);
    if (!dragging) return;
    swallowNextClick();
    const before = snapshot(list);
    const others = siblings.filter((item) => item !== node);
    options.onDrop({
      id: node.dataset.tabId ?? '',
      index: drag.target,
      others: others.map((item) => item.dataset.tabId ?? ''),
    });
    settle(drag, before, options.reduced);
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', end);
  window.addEventListener('pointercancel', end);
}
