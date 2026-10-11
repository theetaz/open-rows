// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { listProfiles } from '@openrows/ipc';
import type { ProfileDto } from '@openrows/ipc';
import { HistoryControls } from './HistoryControls';
/** Settings entry for per-connection local history, without opening a collector. */
export default function MonitorSettingsPane(): React.JSX.Element {
  const [profiles, setProfiles] = useState<ProfileDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    listProfiles().then(
      (value) => {
        if (live) setProfiles(value.profiles);
      },
      (failure: unknown) => {
        if (live) setError(String(failure));
      },
    );
    return () => {
      live = false;
    };
  }, []);
  return (
    <section aria-label="Monitoring settings">
      <h2>Local monitor history</h2>
      {profiles
        .filter((profile) => profile.engine === 'postgres')
        .map((profile) => (
          <section key={profile.id} aria-label={profile.name}>
            <h3>{profile.name}</h3>
            <HistoryControls profileId={profile.id} />
          </section>
        ))}
      {error && <p role="alert">{error}</p>}
    </section>
  );
}
