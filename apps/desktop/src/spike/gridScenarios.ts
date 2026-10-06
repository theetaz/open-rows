// SPDX-License-Identifier: Apache-2.0
/** Sample distant regions without traversing 26 million CSS pixels. */
export const ROWS = 1_000_000;
export const ROW_HEIGHT = 26;
/** Initial visual preview remains sampled and gated. */
export const PREVIEW_MS = 2_000;
const MAX_Y = ROWS * ROW_HEIGHT - 574;
const SEGMENT_MS = 10_000 / 3;
/** Four deterministic scenarios run independently for each option. */
export const SCENARIOS = ['smooth', 'flings', 'horizontal', 'jump'] as const;
interface ScrollTarget {
  x: number;
  y: number;
}
/** Maximum sampled duration, including preview and endpoint delivery. */
export function scenarioDuration(scenario: string): number {
  const active =
    scenario === 'smooth'
      ? 10_000
      : scenario === 'flings'
        ? 12_000
        : scenario === 'horizontal'
          ? 5_000
          : 2_000;
  return PREVIEW_MS + active + (scenario === 'jump' ? 0 : 500);
}
/** Segment teleports need the same event-delivery grace as the initial scroll. */
export function continuousSample(scenario: string, elapsed: number): boolean {
  return (
    scenario !== 'smooth' || elapsed >= 10_000 || elapsed % SEGMENT_MS > 100
  );
}
/** Reuse offsets; motion follows elapsed time so dropped frames remain sampled. */
export function scrollTarget(
  scenario: string,
  elapsed: number,
  target: ScrollTarget,
): void {
  target.x = 0;
  target.y = 0;
  if (scenario === 'smooth') {
    const segment = Math.min(2, Math.floor(elapsed / SEGMENT_MS));
    const start =
      segment === 0 ? 0 : segment === 1 ? 500_000 * ROW_HEIGHT : MAX_Y - 20_000;
    target.y = start + Math.min(20_000, (elapsed - segment * SEGMENT_MS) * 6);
  }
  if (scenario === 'flings') {
    const step = Math.min(119, Math.floor(elapsed / 100));
    target.y =
      step < 60
        ? Math.round((((step * 37) % 60) * 999_000) / 59) * ROW_HEIGHT
        : Math.min(MAX_Y, 500_000 * ROW_HEIGHT + (step - 59) * 574);
  }
  if (scenario === 'horizontal') {
    target.y = 500_000 * ROW_HEIGHT;
    target.x = Math.min(12_000 - 1_040, elapsed * 3);
  }
  if (scenario === 'jump') target.y = 999_000 * ROW_HEIGHT;
}

/** Label the command responsible for the just-completed interval, including initial preview. */
export function scenarioStep(scenario: string, elapsed: number): string {
  if (elapsed < PREVIEW_MS) return 'preview';
  const active = elapsed - PREVIEW_MS;
  if (scenario === 'flings')
    return `fling/page ${String(Math.min(119, Math.floor(active / 100)))}`;
  if (scenario === 'smooth')
    return `smooth segment ${String(Math.min(2, Math.floor(active / SEGMENT_MS)))}`;
  return scenario === 'jump' ? 'jump row 999000' : 'horizontal sweep';
}
