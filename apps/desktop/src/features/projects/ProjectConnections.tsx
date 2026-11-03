// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button } from '@openrows/ui';
import type { ProfileDto, ProjectDto } from '@openrows/ipc';
import { shareProjectConnections } from '@openrows/ipc';
import { reloadProject, reportProjectError } from './projectStore';
import { profilesChanged } from '../connections/useProfiles';
import { t } from './messages';

const Transfer = lazy(() => import('../connections/ConnectionTransferSheet'));

/** Explicitly share selected definitions or preview the project's definitions before import. */
export function ProjectConnections(props: {
  project: ProjectDto;
  profiles: readonly ProfileDto[];
}): React.JSX.Element {
  const [mode, setMode] = useState<'share' | 'import' | null>(null);
  const [ids, setIds] = useState<readonly string[]>([]);
  const [busy, setBusy] = useState(false);
  const share = async (): Promise<void> => {
    setBusy(true);
    try {
      await shareProjectConnections(props.project.root, [...ids]);
      await reloadProject();
      setMode(null);
    } catch (error) {
      reportProjectError(error);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div>
      <Button
        size="sm"
        onClick={() => {
          setMode('share');
        }}
      >
        {t('shareConnections')}
      </Button>
      <Button
        size="sm"
        disabled={!props.project.connectionsFile}
        onClick={() => {
          setMode('import');
        }}
      >
        {t('importProjectConnections')}
      </Button>
      {mode === 'share' && (
        <ShareChoices
          profiles={props.profiles}
          ids={ids}
          onIds={setIds}
          busy={busy}
          onCancel={() => {
            setMode(null);
          }}
          onSave={() => {
            void share();
          }}
        />
      )}
      {mode === 'import' && (
        <Suspense fallback={null}>
          <Transfer
            mode="import"
            profiles={props.profiles}
            projectRoot={props.project.root}
            onClose={() => {
              setMode(null);
            }}
            onImported={profilesChanged}
          />
        </Suspense>
      )}
    </div>
  );
}

function ShareChoices(props: {
  profiles: readonly ProfileDto[];
  ids: readonly string[];
  onIds: (ids: readonly string[]) => void;
  busy: boolean;
  onCancel: () => void;
  onSave: () => void;
}): React.JSX.Element {
  const { ids, busy } = props;
  return (
    <div role="group" aria-label={t('sharedConnectionDefinitions')}>
      <p>{t('chooseNonSecret')}</p>
      {props.profiles.map((profile) => (
        <label key={profile.id} style={{ display: 'block' }}>
          <input
            type="checkbox"
            checked={ids.includes(profile.id)}
            onChange={(event) => {
              props.onIds(
                event.target.checked
                  ? [...ids, profile.id]
                  : ids.filter((id) => id !== profile.id),
              );
            }}
          />{' '}
          {profile.name}
        </label>
      ))}
      <Button
        size="sm"
        onClick={() => {
          props.onCancel();
        }}
      >
        {t('cancel')}
      </Button>
      <Button
        size="sm"
        disabled={busy || !ids.length}
        onClick={() => {
          props.onSave();
        }}
      >
        {t('saveDefinitions')}
      </Button>
    </div>
  );
}
