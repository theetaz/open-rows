// SPDX-License-Identifier: Apache-2.0
import { EngineIcon, isEngineId } from '@openrows/ui';
import { NameFields, ServerFields, SignInFields } from '../ConnectionFields';
import { engineName, isFileEngine } from '../engines';
import { FileAccessFields, FileFields } from '../FileFields';
import { UrlField } from '../UrlField';
import type { ConnectionForm } from '../useConnectionForm';
import { t as profile } from '../messages/profile';
import { FormSection } from './FormSection';
import page from './Flow.module.css';
import { t } from './messages';

/**
 * Step 2: the URL in two-way sync with Server, Sign in and Identity; file
 * engines swap Server and Sign in for the file well and Access.
 */
export function DetailsStep({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { engine } = form.draft;
  const file = isFileEngine(engine);
  return (
    <>
      <h2 className={page.title}>
        {isEngineId(engine) && <EngineIcon engine={engine} size="md" />}
        {t('detailsTitle', { engine: engineName(engine) })}
      </h2>
      <p className={page.lede}>{file ? t('fileLede') : t('detailsLede')}</p>
      <FormSection
        title={t('connectionUrl')}
        icon="link"
        note={file ? undefined : t('urlNote')}
        bare
      >
        <UrlField url={form.url} draft={form.draft} />
      </FormSection>
      {file ? <FileGroups form={form} /> : <ServerGroups form={form} />}
      <FormSection title={t('identity')} icon="tag">
        <NameFields form={form} />
      </FormSection>
    </>
  );
}

function ServerGroups({ form }: { form: ConnectionForm }): React.JSX.Element {
  return (
    <>
      <FormSection title={t('server')} icon="server">
        <ServerFields form={form} />
      </FormSection>
      <FormSection title={t('signIn')} icon="user">
        <SignInFields form={form} />
      </FormSection>
    </>
  );
}

function FileGroups({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { engine } = form.draft;
  return (
    <>
      <FormSection
        title={t('databaseFile')}
        icon="file"
        bare
        note={engineNote(engine)}
      >
        <FileFields form={form} />
      </FormSection>
      <FormSection title={t('access')} icon="shield">
        <FileAccessFields form={form} />
      </FormSection>
    </>
  );
}

/** DuckDB's pending features and libSQL's local limits, said before connecting. */
function engineNote(engine: string): string | undefined {
  if (engine === 'duckdb') return profile('browsingEditingTransactions');
  if (engine === 'libsql-file') return profile('sqliteCompatibleFiles');
  return undefined;
}
