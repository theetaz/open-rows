// SPDX-License-Identifier: Apache-2.0
import { Icon, IconButton } from '@openrows/ui';
import type { RowField } from './row';
import styles from './InspectorHead.module.css';
import { t } from './messages/initial';

/** `Row 9`: a one-based row number as the inspector's title. */
export function rowTitle(number: number): string {
  return t('rowNumber', { number: String(number) });
}

/** The inspector's accessible name. */
export const inspectorLabel = (): string => t('rowDetails');

/** One row as a JSON object: NULL as `null`, EMPTY as `""`, the rest as shown. */
function rowJson(fields: readonly RowField[]): string {
  const entries = fields
    .filter((field) => !field.hidden)
    .map((field) => [
      field.name,
      field.state === 'null' ? null : field.state === 'empty' ? '' : field.text,
    ]);
  return JSON.stringify(Object.fromEntries(entries), null, 2);
}

/**
 * The inspector's heading (the prototype's `.insp-head`): `Row 9` and the
 * table, then Copy row as JSON and Hide details when they apply.
 */
export function InspectorHead(props: {
  title: string;
  subtitle: string;
  /** The row's fields, offering Copy row as JSON. */
  fields?: readonly RowField[] | undefined;
  onHide?: (() => void) | undefined;
}): React.JSX.Element {
  const { fields, onHide } = props;
  return (
    <header className={styles.head}>
      <span className={styles.title}>{props.title}</span>
      <span className={styles.subtitle}>{props.subtitle}</span>
      {fields && fields.length > 0 && (
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={t('copyRowJson')}
          title={t('copyRowJson')}
          onClick={() => {
            void navigator.clipboard.writeText(rowJson(fields));
          }}
        >
          <Icon name="copy" />
        </IconButton>
      )}
      {onHide && (
        <IconButton
          size="sm"
          variant="ghost"
          aria-label={t('hideDetails')}
          title={t('hideDetails')}
          onClick={onHide}
        >
          <Icon name="close" />
        </IconButton>
      )}
    </header>
  );
}
