// SPDX-License-Identifier: Apache-2.0
import { createContext, useContext, useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { readTheme, saveTheme } from './storage';
import type { ThemeMode } from './storage';

type Theme = {
  mode: ThemeMode;
  resolved: 'light' | 'dark';
  setMode: (mode: ThemeMode) => void;
};
const ThemeContext = createContext<Theme | null>(null);
const query = '(prefers-color-scheme: dark)';
/** Applies one root attribute; CSS performs the system-theme and material changes. */
export function ThemeProvider({
  children,
}: {
  children: ReactNode;
}): React.JSX.Element {
  const [mode, updateMode] = useState<ThemeMode>(readTheme);
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
  useEffect(() => {
    document.documentElement.dataset.theme = mode;
  }, [mode]);
  function setMode(next: ThemeMode): void {
    updateMode(next);
    saveTheme(next);
  }
  const resolved = mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;
  return (
    <ThemeContext value={{ mode, resolved, setMode }}>{children}</ThemeContext>
  );
}
/** Read or change appearance within a ThemeProvider. */
export function useTheme(): Theme {
  const theme = useContext(ThemeContext);
  if (!theme) throw new Error('useTheme requires ThemeProvider');
  return theme;
}
