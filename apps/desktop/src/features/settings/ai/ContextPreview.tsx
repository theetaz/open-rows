// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import { previewAiContext } from '@openrows/ipc';
import type {
  ContextBundle,
  ContextPreview as Preview,
  ProviderConfig,
} from '@openrows/ipc';
import styles from './Ai.module.css';
type Props = {
  id: string;
  providers: readonly ProviderConfig[];
  disabled: boolean;
  bundle?: ContextBundle;
};
/** Reusable offline exact preview; it accepts the same selected bundle as later assistant surfaces. */
export function ContextPreview(props: Props): React.JSX.Element {
  const [provider, setProvider] = useState(props.providers[0]?.id ?? '');
  const [query, setQuery] = useState('');
  const [ddl, setDdl] = useState('');
  const [result, setResult] = useState<Preview | null>(null);
  const [failure, setFailure] = useState('');
  const [busy, setBusy] = useState(false);
  const bundle = props.bundle ?? {
    schema: ddl ? [{ name: 'selected', ddl }] : [],
    query: query || null,
    plan: [],
    stats: [],
    samples: [],
  };
  const preview = async (): Promise<void> => {
    setBusy(true);
    setResult(null);
    setFailure('');
    try {
      setResult(await previewAiContext(props.id, provider, bundle));
    } catch (error) {
      setFailure(
        error instanceof Error
          ? error.message
          : 'Context refused by the sharing policy.',
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <section aria-label="Context preview" className={styles.form}>
      <PreviewIntro />
      <SelectField
        disabled={busy}
        label="Preview provider"
        value={provider}
        options={[
          { value: '', label: 'Choose provider' },
          ...props.providers.map((item) => ({
            value: item.id,
            label: item.name,
          })),
        ]}
        onValueChange={(id) => {
          setProvider(id);
          setResult(null);
        }}
      />
      {!props.bundle && (
        <ContextInputs
          disabled={busy}
          query={query}
          ddl={ddl}
          onQuery={(value) => {
            setQuery(value);
            setResult(null);
          }}
          onDdl={(value) => {
            setDdl(value);
            setResult(null);
          }}
        />
      )}
      <Button
        type="button"
        disabled={props.disabled || busy || !provider}
        onClick={() => {
          void preview();
        }}
      >
        Preview context
      </Button>
      {failure && <p role="alert">{failure}</p>}
      {result && <PreviewPayload result={result} />}
    </section>
  );
}

function ContextInputs({
  disabled,
  query,
  ddl,
  onQuery,
  onDdl,
}: {
  disabled: boolean;
  query: string;
  ddl: string;
  onQuery: (value: string) => void;
  onDdl: (value: string) => void;
}): React.JSX.Element {
  return (
    <>
      <label className={styles.form}>
        Schema definitions
        <textarea
          disabled={disabled}
          aria-label="Schema definitions"
          value={ddl}
          maxLength={65536}
          onChange={(event) => {
            onDdl(event.target.value);
          }}
        />
      </label>
      <label className={styles.form}>
        Query SQL
        <textarea
          disabled={disabled}
          aria-label="Query SQL"
          value={query}
          maxLength={65536}
          onChange={(event) => {
            onQuery(event.target.value);
          }}
        />
      </label>
    </>
  );
}

function PreviewPayload({ result }: { result: Preview }): React.JSX.Element {
  return (
    <div role="status">
      <p>
        {result.bytes} bytes · ≤ {result.tokens} estimated tokens ·{' '}
        {result.categories.join(', ') || 'empty'}
        {result.truncated ? ' · Truncated to sharing limits' : ''}
      </p>
      <pre aria-label="Exact context payload" className={styles.payload}>
        {result.payload}
      </pre>
    </div>
  );
}

function PreviewIntro(): React.JSX.Element {
  return (
    <>
      <h4>What will be sent</h4>
      <p className={styles.notice}>
        Offline preview only. Paste selected definitions or SQL below; nothing
        is read from your database or sent to a provider. Save policy changes
        before previewing.
      </p>
    </>
  );
}
