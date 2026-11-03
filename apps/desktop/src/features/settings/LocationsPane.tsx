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
import { t } from './messages/lazyGeneral';

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
      setMessage(t('restartOpenrowsCopy'));
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    }
  };
  return (
    <section aria-label={t('locations')}>
      <h3>{t('locations')}</h3>
      {(['data', 'history', 'logs'] as const).map((key) => (
        <SettingRow
          key={key}
          title={t('keyLocation', { key })}
          description={locations?.[key] ?? t('availableNativeApp')}
        >
          <code>{locations?.[key] ?? 'Native app only'}</code>
        </SettingRow>
      ))}
      <SettingRow
        title={t('changeDataDirectory')}
        description={t('chooseEmptyWritable')}
      >
        <Button
          onClick={() => {
            void choose();
          }}
        >
          {t('chooseDataFolder')}
        </Button>
      </SettingRow>
      <p>{t('doNotChoose')}</p>
      {locations?.pending && (
        <p role="status">
          {t('restartRequiredPending', { pending: locations.pending })}
        </p>
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
      title={t('copyLocalData')}
      onClose={() => {
        close();
      }}
    >
      <p>{t('copySettingsProfiles', { target })}</p>
      <SheetActions>
        <Button
          onClick={() => {
            close();
          }}
        >
          {t('cancel')}
        </Button>
        <Button
          variant="primary"
          onClick={() => {
            void apply();
          }}
        >
          {t('copyNextRestart')}
        </Button>
      </SheetActions>
    </Sheet>
  );
}
