// SPDX-License-Identifier: Apache-2.0
import { TextField } from '@openrows/ui';
import { NO_MONGO_OPTIONS } from './form';
import { SwitchRow } from './SwitchRow';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';

/** MongoDB discovery and authentication database, shared with URL fields. */
export function MongoFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const value = form.draft.mongodb ?? NO_MONGO_OPTIONS;
  return (
    <>
      <SwitchRow
        label={t('dnsSeedList')}
        detail={t('connectMongodbSrv')}
        checked={value.srv}
        onChange={(srv) => {
          form.update({
            mongodb: { ...value, srv, direct: srv ? false : value.direct },
            tlsMode: srv ? 'verify-full' : form.draft.tlsMode,
          });
        }}
      />
      {!value.srv && (
        <SwitchRow
          label={t('directConnection')}
          detail={t('talkOnlyServer')}
          checked={value.direct}
          onChange={(direct) => {
            form.update({ mongodb: { ...value, direct } });
          }}
        />
      )}
      <TextField
        className={styles.row}
        label={t('authDatabase')}
        placeholder={t('databaseUrlAdmin')}
        value={value.authSource}
        onChange={(event) => {
          form.update({
            mongodb: { ...value, authSource: event.target.value },
          });
        }}
      />
    </>
  );
}
