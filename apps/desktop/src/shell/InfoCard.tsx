// SPDX-License-Identifier: Apache-2.0
import { useAppInfo } from './useAppInfo';
import styles from './InfoCard.module.css';

/** Display Rust application metadata with explicit loading and error states. */
export function InfoCard(): React.JSX.Element {
  const state = useAppInfo();

  return (
    <section className={styles.card} aria-label="Application information">
      {state.status === 'loading' && (
        <p role="status">Loading application information…</p>
      )}
      {state.status === 'error' && (
        <p role="alert">
          Unable to load application information: {state.message}
        </p>
      )}
      {state.status === 'success' && (
        <>
          <h1>{state.info.name}</h1>
          <p>Version {state.info.version}</p>
          <p>{state.info.buildProfile}</p>
        </>
      )}
    </section>
  );
}
