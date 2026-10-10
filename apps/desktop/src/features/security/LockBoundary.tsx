// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useRef } from 'react';
import { lockApp, securityActivity } from '@openrows/ipc';
import { commandForEvent } from '../commands';
import { useSecurity } from './useSecurity';
import { useSuspendedDialogs } from './useSuspendedDialogs';
import './Security.module.css';
const LockScreen = lazy(() =>
  import('./LockScreen').then((m) => ({ default: m.LockScreen })),
);
/** Hide every data surface and portal, preserving background queries until unlock. */
export function LockBoundary(props: {
  children: React.ReactNode;
}): React.JSX.Element {
  const status = useSecurity();
  const current = useRef(status);
  useSuspendedDialogs(status.locked);
  useEffect(() => {
    current.current = status;
    if (status.locked) {
      document.getSelection()?.removeAllRanges();
      (document.activeElement as HTMLElement | null)?.blur();
    }
    document.body.toggleAttribute('data-openrows-locked', status.locked);
    return () => {
      document.body.removeAttribute('data-openrows-locked');
    };
  }, [status]);
  useEffect(() => {
    const key = (e: KeyboardEvent): void => {
      if (current.current.locked && (e.metaKey || e.ctrlKey || e.altKey)) {
        e.preventDefault();
        e.stopImmediatePropagation();
        return;
      }
      if (
        !current.current.locked &&
        current.current.available &&
        commandForEvent(e, ['lock-app']) === 'lock-app'
      ) {
        e.preventDefault();
        e.stopImmediatePropagation();
        void lockApp();
      }
    };
    let last = 0;
    const activity = (e: Event): void => {
      if (
        !e.isTrusted ||
        current.current.locked ||
        performance.now() - last < 1000
      )
        return;
      last = performance.now();
      void securityActivity();
    };
    window.addEventListener('keydown', key, true);
    const inputs = ['keydown', 'pointerdown', 'pointermove', 'wheel', 'input'];
    inputs.forEach((name) => {
      window.addEventListener(name, activity, true);
    });
    return () => {
      window.removeEventListener('keydown', key, true);
      inputs.forEach((name) => {
        window.removeEventListener(name, activity, true);
      });
    };
  }, []);
  return (
    <>
      <div hidden={status.locked} inert={status.locked}>
        {props.children}
      </div>
      {status.locked && (
        <Suspense fallback={<p role="status">Checking application lock…</p>}>
          <LockScreen />
        </Suspense>
      )}
    </>
  );
}
