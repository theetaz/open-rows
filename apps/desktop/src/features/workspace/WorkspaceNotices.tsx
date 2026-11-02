// SPDX-License-Identifier: Apache-2.0
import styles from './Workspace.module.css';

/** A problem opening a file from Finder, a link or the command line. */
export function FileNotice({
  notice,
}: {
  notice: string | null | undefined;
}): React.JSX.Element | null {
  if (!notice) return null;
  return (
    <p className={styles.fileNotice} role="alert">
      {notice}
    </p>
  );
}

/** Screen-reader confirmation that the catalog was refreshed. */
export function RefreshAnnouncement({
  refreshes,
}: {
  refreshes: number;
}): React.JSX.Element {
  return (
    <span className={styles.announcement} role="status">
      {refreshes ? `Catalog refreshed (${String(refreshes)})` : ''}
    </span>
  );
}
