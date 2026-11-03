// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import type {
  ConnectionConflictDto,
  ImportDecisionDto,
  ImportPreview,
} from '@openrows/ipc';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/lazyTransfer';
const labels: Record<ConnectionConflictDto, string> = {
  new: t('new'),
  sameName: t('sameNameExists'),
  identical: t('identicalSkip'),
};
const choices = [
  { value: 'skip' as const, label: t('skip') },
  { value: 'replace' as const, label: t('replace') },
  { value: 'keepBoth' as const, label: t('keepBothSuffix') },
];
/** Per-row decisions and optional keychain save; passwords never cross into this sheet. */
export function ConnectionImportPreview(props: {
  preview: ImportPreview;
  decisions: ImportDecisionDto[];
  onDecision: (index: number, patch: Partial<ImportDecisionDto>) => void;
}): React.JSX.Element {
  return (
    <div className={styles.stack}>
      {props.preview.warnings.map((warning, index) => (
        <p key={index} role="note">
          {warning}
        </p>
      ))}
      {props.preview.rows.map((row, index) => (
        <div className={styles.stack} key={row.profile.id}>
          <strong>{row.profile.name}</strong>
          <small>
            {row.profile.engine} · {row.profile.host}:{row.profile.port} ·{' '}
            {labels[row.conflict]}
          </small>
          {row.conflict === 'sameName' && (
            <SelectField
              label={t('actionName', { name: row.profile.name })}
              options={choices}
              value={props.decisions[index]?.choice ?? 'skip'}
              onValueChange={(choice) => {
                props.onDecision(index, { choice });
              }}
            />
          )}
          {row.hasPassword && row.conflict !== 'identical' && (
            <label>
              <input
                type="checkbox"
                checked={props.decisions[index]?.savePassword ?? false}
                onChange={(event) => {
                  props.onDecision(index, {
                    savePassword: event.target.checked,
                  });
                }}
              />
              {t('savePasswordName', { name: row.profile.name })}
            </label>
          )}
        </div>
      ))}
      {props.preview.rows.length === 0 && <p>{t('noConnectionsFound')}</p>}
      <small className={styles.hint}>{t('importedCommandsUntrusted')}</small>
    </div>
  );
}
