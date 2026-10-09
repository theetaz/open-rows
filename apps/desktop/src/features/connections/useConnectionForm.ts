// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import {
  createProfile,
  revealSecret,
  testConnection,
  updateProfile,
} from '@openrows/ipc';
import type {
  EngineDto,
  EnvKindDto,
  ProfileDto,
  SecretFieldDto,
  TestReport,
} from '@openrows/ipc';
import {
  draftFromProfile,
  emptyDraft,
  toInput,
  validate,
  withEnvironment,
  withPassword,
  withPatch,
} from './form';
import type { Draft, DraftErrors } from './form';
import type { SshDraft, TlsDraft } from './routeForm';
import type { ProxyDraft } from './proxyForm';
import { useUrlField } from './useUrlField';
import type { UrlField } from './useUrlField';

export type TestState =
  | { status: 'idle' }
  | { status: 'testing' }
  /** The test ran: every step, and the server or the failing step's error. */
  | { status: 'done'; report: TestReport }
  /** The test could not start (invalid input). */
  | { status: 'error'; message: string };

export type ConnectionForm = {
  draft: Draft;
  profileId: string | null;
  errors: DraftErrors;
  test: TestState;
  saving: boolean;
  saveError: string | null;
  /** The connection URL, kept in sync with the fields both ways. */
  url: UrlField;
  update: (patch: Partial<Draft>) => void;
  /** Edit the SSH section (not part of the URL). */
  updateSsh: (patch: Partial<SshDraft>) => void;
  /** Edit the TLS certificate files (not part of the URL). */
  updateTls: (patch: Partial<TlsDraft>) => void;
  /** Edit the proxy section (not part of the URL). */
  updateProxy: (patch: Partial<ProxyDraft>) => void;
  setEnvironment: (kind: EnvKindDto) => void;
  setPassword: (value: string) => void;
  runTest: () => Promise<void>;
  save: () => Promise<void>;
  /** Reads a saved secret for an explicit reveal; absent for a new connection. */
  reveal: (field: SecretFieldDto) => (() => Promise<string>) | undefined;
};

/** Fields that appear in the connection URL. */
const URL_FIELDS = new Set<string>([
  'filePath',
  'fileReadOnly',
  'host',
  'port',
  'database',
  'user',
  'tlsMode',
  'password',
  'passwordMode',
]);

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** Form state for the new/edit connection sheet; errors show after the first Test or Save. */
export function useConnectionForm(
  profile: ProfileDto | undefined,
  onSaved: (profile: ProfileDto) => void,
  engine: EngineDto = 'postgres',
): ConnectionForm {
  const [draft, setDraft] = useState(() =>
    profile ? draftFromProfile(profile) : emptyDraft(engine),
  );
  const [submitted, setSubmitted] = useState(false);
  const url = useUrlField(draft, setDraft);
  const checked = (): boolean => {
    setSubmitted(true);
    return Object.keys(validate(draft)).length === 0;
  };
  const test = useTest(profile?.id ?? null, draft, checked);
  const saving = useSave(profile, draft, checked, onSaved);
  return {
    draft,
    profileId: profile?.id ?? null,
    errors: submitted ? validate(draft) : {},
    test: test.state,
    saving: saving.busy,
    saveError: saving.error,
    url,
    ...editors(setDraft, url),
    runTest: test.run,
    save: saving.run,
    reveal: (field) =>
      profile ? () => revealSecret(profile.id, field) : undefined,
  };
}

type Editors = Pick<
  ConnectionForm,
  | 'update'
  | 'updateSsh'
  | 'updateTls'
  | 'updateProxy'
  | 'setEnvironment'
  | 'setPassword'
>;

/** Draft edits; fields that appear in the URL show the fields' URL again. */
function editors(
  setDraft: Dispatch<SetStateAction<Draft>>,
  url: UrlField,
): Editors {
  return {
    update: (patch) => {
      setDraft((current) => withPatch(current, patch));
      if (Object.keys(patch).some((key) => URL_FIELDS.has(key))) url.release();
    },
    updateSsh: (patch) => {
      setDraft((current) => ({
        ...current,
        ssh: { ...current.ssh, ...patch },
      }));
    },
    updateTls: (patch) => {
      setDraft((current) => ({
        ...current,
        tls: { ...current.tls, ...patch },
      }));
    },
    updateProxy: (patch) => {
      setDraft((current) => ({
        ...current,
        proxy: { ...current.proxy, ...patch },
      }));
    },
    setEnvironment: (kind) => {
      setDraft((current) => withEnvironment(current, kind));
    },
    setPassword: (value) => {
      setDraft((current) => withPassword(current, value));
      url.release();
    },
  };
}

function useTest(
  profileId: string | null,
  draft: Draft,
  checked: () => boolean,
): { state: TestState; run: () => Promise<void> } {
  const [state, setState] = useState<TestState>({ status: 'idle' });
  const run = async (): Promise<void> => {
    if (!checked()) return;
    setState({ status: 'testing' });
    try {
      const report = await testConnection(profileId, toInput(draft));
      setState({ status: 'done', report });
    } catch (error) {
      setState({ status: 'error', message: message(error) });
    }
  };
  return { state, run };
}

function useSave(
  profile: ProfileDto | undefined,
  draft: Draft,
  checked: () => boolean,
  onSaved: (profile: ProfileDto) => void,
): { busy: boolean; error: string | null; run: () => Promise<void> } {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (): Promise<void> => {
    if (!checked() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const input = toInput(draft);
      onSaved(
        profile
          ? await updateProfile(profile.id, input)
          : await createProfile(input),
      );
    } catch (failure) {
      setError(message(failure));
      setBusy(false);
    }
  };
  return { busy, error, run };
}
