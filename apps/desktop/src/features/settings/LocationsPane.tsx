// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { Sheet, SheetActions } from '@openrows/ui/sheet';
import {
  chooseDataLocation,
  getDataLocations,
  scheduleDataLocation,
} from '@openrows/ipc';
import type { LocationsDto } from '@openrows/ipc';
import { SettingRow } from '../appearance/InterfaceSettings';

/** Explicit history-copy consent and restart-only relocation; keychain stays fixed. */
export function LocationsPane(): React.JSX.Element {
  const [locations, setLocations] = useState<LocationsDto | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [message, setMessage] = useState('');
  useEffect(() => {
    getDataLocations().then(setLocations, (error: unknown) => {
      setMessage(String(error));
    });
  }, []);
  const choose = async (): Promise<void> => {
    try {
      setTarget(await chooseDataLocation());
    } catch (error) {
      setMessage(String(error));
    }
  };
  const apply = async (): Promise<void> => {
    if (!target) return;
    try {
      await scheduleDataLocation(target);
      setTarget(null);
      setLocations(await getDataLocations());
      setMessage(
        'Restart OpenRows to copy data and switch. The original remains available if copying fails.',
      );
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  };
  return (
    <section aria-label="Locations">
      <h3>Locations</h3>
      {(['data', 'history', 'logs'] as const).map((key) => (
        <SettingRow
          key={key}
          title={`${key} location`}
          description={locations?.[key] ?? 'Available in the native app'}
        >
          <code>{locations?.[key] ?? 'Native app only'}</code>
        </SettingRow>
      ))}
      <SettingRow
        title="Change data directory"
        description="Choose an empty writable local folder outside app bundles and cloud sync. Copy includes local history; secrets remain in the OS keychain."
      >
        <Button
          onClick={() => {
            void choose();
          }}
        >
          Choose data folder
        </Button>
      </SettingRow>
      <p>
        Do not choose any synchronised folder: history may contain private SQL.
        Cloud folder detection is heuristic.
      </p>
      {locations?.pending && (
        <p role="status">Restart required: {locations.pending}</p>
      )}
      {locations?.failure && <p role="alert">{locations.failure}</p>}
      {message && <p role="status">{message}</p>}
      {target && (
        <MoveDialog
          target={target}
          close={() => {
            setTarget(null);
          }}
          apply={apply}
        />
      )}
    </section>
  );
}

/** Consent includes local history explicitly and leaves restart under user control. */
function MoveDialog({
  target,
  close,
  apply,
}: {
  target: string;
  close: () => void;
  apply: () => Promise<void>;
}): React.JSX.Element {
  return (
    <Sheet
      title="Copy local data on restart?"
      onClose={() => {
        close();
      }}
    >
      <p>
        Copy settings, profiles and local history to {target}? Keychain secrets
        and logs stay in their current locations. This is a local copy; history
        is never silently synchronised.
      </p>
      <SheetActions>
        <Button
          onClick={() => {
            close();
          }}
        >
          Cancel
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            void apply();
          }}
        >
          Copy on next restart
        </Button>
      </SheetActions>
    </Sheet>
  );
}
