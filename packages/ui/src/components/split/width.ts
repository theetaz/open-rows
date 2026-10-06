// SPDX-License-Identifier: Apache-2.0
/** Persisted panel bounds and reset width, in CSS pixels. */
export type PanelRange = { min: number; max: number; initial: number };
/** Reject invalid widths and clamp every input path to the same limits. */
export function boundedWidth(value: number, range: PanelRange): number {
  return Number.isFinite(value)
    ? Math.max(range.min, Math.min(range.max, value))
    : range.initial;
}
/** Storage is optional; malformed or unavailable preferences use the reset width. */
export function readWidth(key: string, range: PanelRange): number {
  try {
    const value = localStorage.getItem(key);
    return value === null ? range.initial : boundedWidth(Number(value), range);
  } catch {
    return range.initial;
  }
}
/** A blocked storage write must not interrupt a resize gesture. */
export function writeWidth(key: string, width: number): void {
  try {
    localStorage.setItem(key, String(width));
  } catch {
    /* In-memory width remains usable. */
  }
}
