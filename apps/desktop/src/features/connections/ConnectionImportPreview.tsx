// SPDX-License-Identifier: Apache-2.0
import { SelectField } from '@openrows/ui';
import type {
  ConnectionConflictDto,
  ImportDecisionDto,
  ImportPreview,
} from '@openrows/ipc';
import styles from './ConnectionSheet.module.css';
const labels: Record<ConnectionConflictDto, string> = {
  new: 'New',
  sameName: 'Same name exists',
  identical: 'Identical (skip)',
};
const choices = [
  { value: 'skip' as const, label: 'Skip' },
  { value: 'replace' as const, label: 'Replace' },
  { value: 'keepBoth' as const, label: 'Keep both with suffix' },
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
              label={`Action for ${row.profile.name}`}
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
              Save password for {row.profile.name} in keychain
            </label>
          )}
        </div>
      ))}
      {props.preview.rows.length === 0 && <p>No connections found.</p>}
      <small className={styles.hint}>
        Imported commands are untrusted. Their exact program and arguments must
        be confirmed before execution.
      </small>
    </div>
  );
}
