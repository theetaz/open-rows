// SPDX-License-Identifier: Apache-2.0
import { localSetting } from './localSetting';

/** Row height of the sidebar trees. */
export type Density = 'compact' | 'regular' | 'comfortable';

/** Seconds between connection keepalive probes; `0` turns them off. */
type KeepaliveSeconds = 0 | 15 | 30 | 60 | 120;

/** Every interval the settings offer (docs/14 §4: 30 s by default). */
export const KEEPALIVE_CHOICES: readonly KeepaliveSeconds[] = [
  15, 30, 60, 120, 0,
];

/** App-wide preferences outside appearance and the editor. */
type GeneralSettings = {
  /** Reopen the previous connections, databases and tabs on launch. */
  restoreSession: boolean;
  density: Density;
  /** Turn off animations even when the system allows them. */
  reduceMotion: boolean;
  /**
   * How often idle connections are checked so a dead one is noticed and
   * reopened; read by the core from settings.json.
   */
  keepaliveSeconds: KeepaliveSeconds;
};

const defaultGeneralSettings: GeneralSettings = {
  restoreSession: true,
  density: 'regular',
  reduceMotion: false,
  keepaliveSeconds: 30,
};

/** Key in web storage (and `general` in settings.json). */
export const GENERAL_KEY = 'openrows.general';

const DENSITIES: readonly Density[] = ['compact', 'regular', 'comfortable'];

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseGeneral(stored: unknown): GeneralSettings {
  if (typeof stored !== 'object' || stored === null)
    return defaultGeneralSettings;
  const value = stored as Record<string, unknown>;
  const flag = (key: 'restoreSession' | 'reduceMotion'): boolean =>
    typeof value[key] === 'boolean' ? value[key] : defaultGeneralSettings[key];
  return {
    restoreSession: flag('restoreSession'),
    reduceMotion: flag('reduceMotion'),
    density: DENSITIES.includes(value.density as Density)
      ? (value.density as Density)
      : defaultGeneralSettings.density,
    keepaliveSeconds: KEEPALIVE_CHOICES.includes(
      value.keepaliveSeconds as KeepaliveSeconds,
    )
      ? (value.keepaliveSeconds as KeepaliveSeconds)
      : defaultGeneralSettings.keepaliveSeconds,
  };
}

export const generalSettings = localSetting(GENERAL_KEY, parseGeneral);

/** Merge a change into the general settings. */
export function updateGeneral(change: Partial<GeneralSettings>): void {
  generalSettings.set({ ...generalSettings.get(), ...change });
}

/** Reflect density and reduced motion on the root element for CSS. */
export function applyGeneral(settings: GeneralSettings): void {
  const root = document.documentElement;
  root.dataset.density = settings.density;
  if (settings.reduceMotion) root.dataset.motion = 'reduced';
  else delete root.dataset.motion;
}
