// SPDX-License-Identifier: Apache-2.0
type Point = { x: number; y: number };

function side(p: Point, a: Point, b: Point): number {
  return (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
}

/** Whether `p` lies inside (or on) the triangle `a`, `b`, `c`. */
function inTriangle(p: Point, a: Point, b: Point, c: Point): boolean {
  const d1 = side(p, a, b);
  const d2 = side(p, b, c);
  const d3 = side(p, c, a);
  const negative = d1 < 0 || d2 < 0 || d3 < 0;
  const positive = d1 > 0 || d2 > 0 || d3 > 0;
  return !(negative && positive);
}

/**
 * The safe triangle: while the pointer travels from `from` towards an open
 * submenu (`submenu`, beside `parent`), other parent rows must not take over.
 * The triangle runs from the last pointer position to the submenu's near
 * corners, 6 px beyond its top and bottom.
 */
export function aimsAtSubmenu(
  from: Point,
  to: Point,
  submenu: Pick<DOMRect, 'left' | 'right' | 'top' | 'bottom'>,
  parent: Pick<DOMRect, 'left'>,
): boolean {
  const nearX = submenu.left > parent.left ? submenu.left : submenu.right;
  return inTriangle(
    to,
    from,
    { x: nearX, y: submenu.top - 6 },
    { x: nearX, y: submenu.bottom + 6 },
  );
}
