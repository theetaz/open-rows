// SPDX-License-Identifier: Apache-2.0
import {
  createContext,
  useContext,
  useEffect,
  useState,
  useMemo,
  useLayoutEffect,
} from 'react';
import type { ReactNode } from 'react';
import { readTheme, saveTheme } from './storage';
import type { ThemeMode } from './storage';
import {
  readAppearance,
  saveAppearance,
  parseAppearance,
  fontFamily,
} from './appearance';
import type { Appearance } from './appearance';
import { appearanceCss, resolvePalette } from './resolveAppearance';
import { useAppearanceMedia } from './useAppearanceMedia';

type Theme = {
  mode: ThemeMode;
  resolved: 'light' | 'dark';
  setMode: (mode: ThemeMode) => void;
  appearance: Appearance;
  setAppearance: (patch: Partial<Appearance>) => void;
  gridAppearance: ReturnType<typeof gridAppearance>;
};
const ThemeContext = createContext<Theme | null>(null);
const query = '(prefers-color-scheme: dark)';
/** Applies one root attribute; CSS performs the system-theme and material changes. */
export function ThemeProvider({
  children,
  onChange,
  revision = 0,
}: {
  children: ReactNode;
  /** Called after the user changed the scheme or appearance (persisted copies). */
  onChange?: (() => void) | undefined;
  /**
   * Bump after the stored scheme and appearance were replaced from outside
   * (an edited settings file): both are read again, without `onChange`.
   */
  revision?: number;
}): React.JSX.Element {
  const [appearance, updateAppearance] = useState(readAppearance);
  const [mode, updateMode] = useState<ThemeMode>(readTheme);
  const [seen, setSeen] = useState(revision);
  if (seen !== revision) {
    setSeen(revision);
    updateAppearance(readAppearance());
    updateMode(readTheme());
  }

  useLayoutEffect(() => {
    const style = document.createElement('style');
    style.dataset.openrowsAppearance = '';
    style.textContent = appearanceCss(appearance);
    document.head.append(style);
    return () => {
      style.remove();
    };
  }, [appearance]);
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia(query).matches,
  );
  useEffect(() => {
    const media = window.matchMedia(query);
    const listener = (event: MediaQueryListEvent): void => {
      setSystemDark(event.matches);
    };
    media.addEventListener('change', listener);
    return () => {
      media.removeEventListener('change', listener);
    };
  }, []);
  const media = useAppearanceMedia();
  useEffect(() => {
    document.documentElement.dataset.theme = mode;
  }, [mode]);
  function setMode(next: ThemeMode): void {
    updateMode(next);
    saveTheme(next);
    onChange?.();
  }
  function setAppearance(patch: Partial<Appearance>): void {
    updateAppearance((previous) => {
      const next = parseAppearance({ ...previous, ...patch });
      saveAppearance(next);
      return next;
    });
    onChange?.();
  }
  const resolved = mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;
  const grid = useMemo(
    () => gridAppearance(appearance, resolved, media),
    [appearance, resolved, media],
  );
  return (
    <ThemeContext
      value={{
        mode,
        resolved,
        setMode,
        appearance,
        setAppearance,
        gridAppearance: grid,
      }}
    >
      {children}
    </ThemeContext>
  );
}
/** Read or change appearance within a ThemeProvider. */
export function useTheme(): Theme {
  const theme = useContext(ThemeContext);
  if (!theme) throw new Error('useTheme requires ThemeProvider');
  return theme;
}

function gridAppearance(
  settings: Appearance,
  mode: 'light' | 'dark',
  media: ReturnType<typeof useAppearanceMedia>,
): {
  colours: ReturnType<typeof resolvePalette>;
  fontFamily: string;
  fontSize: number;
} {
  const colours = resolvePalette(settings, mode);
  if (media.contrast) {
    colours.separator = mode === 'light' ? '#606060' : '#C0C0C0';
    colours['control-fill'] = mode === 'light' ? '#E4E4E4' : '#383838';
  }
  return {
    colours,
    fontFamily: fontFamily('mono', settings.monoFont),
    fontSize: settings.monoSize,
  };
}
