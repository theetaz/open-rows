// SPDX-License-Identifier: Apache-2.0
/** Public update-feed configuration injected only by release builds (docs/16 §8). */
interface ImportMetaEnv {
  readonly VITE_OPENROWS_UPDATE_STABLE?: string;
  readonly VITE_OPENROWS_UPDATE_BETA?: string;
  readonly VITE_OPENROWS_UPDATE_KEY?: string;
}
