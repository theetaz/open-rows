// SPDX-License-Identifier: Apache-2.0
/**
 * Explain and Explain Analyze with a plan visualiser and hotspots (M2-06,
 * FR-EDIT-07). The visualiser loads on first use, outside the startup bundle.
 */
export { useExplain } from './useExplain';
export type { ExplainControls } from './useExplain';
export { ExplainSheet } from './ExplainConfirm';
export { explainResultTab } from './ExplainResult';
export type { ExtraResultTab } from './ExplainResult';
