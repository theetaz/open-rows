// SPDX-License-Identifier: Apache-2.0
import { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';
import { Button } from './Button';
import styles from './ErrorBoundary.module.css';
import { t } from '../messages';

type Props = {
  children: ReactNode;
  /** Reported once per caught render error, e.g. to the diagnostics log. */
  onError?: ((error: unknown, info: ErrorInfo) => void) | undefined;
  /** Defaults to reloading the window. */
  onReload?: (() => void) | undefined;
};

/**
 * A render error below this boundary shows a plain "Something went wrong"
 * pane with a Reload button instead of blanking the window.
 */
export class ErrorBoundary extends Component<Props, { failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  override componentDidCatch(error: unknown, info: ErrorInfo): void {
    this.props.onError?.(error, info);
  }

  override render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    const reload =
      this.props.onReload ??
      (() => {
        window.location.reload();
      });
    return (
      <div role="alert" className={styles.pane} data-tauri-drag-region>
        <h1>{t('somethingWentWrong')}</h1>
        <p>{t('reloadToContinue')}</p>
        <Button variant="primary" onClick={reload}>
          {t('reload')}
        </Button>
      </div>
    );
  }
}
