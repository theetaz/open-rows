// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { SelectField } from '@openrows/ui';
import { useProfiles } from '../../connections/useProfiles';
import ConnectionSharing from './ConnectionSharing';
import { AuditViewer } from './AuditViewer';
import { t } from '../messages/lazyAi';
/** Lazy settings surface for every saved connection's sharing policy and local audit. */
export default function SharingPane(): React.JSX.Element {
  const { profiles, problem } = useProfiles();
  const [id, setId] = useState('');
  const profile = profiles.find((item) => item.id === id);
  return (
    <section aria-label={t('aiDataSharing')}>
      <h3>{t('dataSharing')}</h3>
      <p>{t('everyConnectionStarts')}</p>
      {problem && <p role="alert">{problem}</p>}
      <SelectField
        label={t('sharingConnection')}
        value={id}
        options={[
          { value: '', label: t('chooseConnection') },
          ...profiles.map((item) => ({ value: item.id, label: item.name })),
        ]}
        onValueChange={setId}
      />
      {profile && (
        <ConnectionSharing
          key={id}
          id={id}
          production={profile.env.kind === 'prod'}
        />
      )}
      <AuditViewer />
    </section>
  );
}
