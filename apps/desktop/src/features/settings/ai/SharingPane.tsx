// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { SelectField } from '@openrows/ui';
import { useProfiles } from '../../connections/useProfiles';
import ConnectionSharing from './ConnectionSharing';
import { AuditViewer } from './AuditViewer';
/** Lazy settings surface for every saved connection's sharing policy and local audit. */
export default function SharingPane(): React.JSX.Element {
  const { profiles, problem } = useProfiles();
  const [id, setId] = useState('');
  const profile = profiles.find((item) => item.id === id);
  return (
    <section aria-label="AI data sharing">
      <h3>Data sharing</h3>
      <p>
        Every connection starts at None. Policies are device-local and never
        copied with exported or duplicated connections.
      </p>
      {problem && <p role="alert">{problem}</p>}
      <SelectField
        label="Sharing connection"
        value={id}
        options={[
          { value: '', label: 'Choose connection' },
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
