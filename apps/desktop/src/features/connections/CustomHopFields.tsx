// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import type { HelperHop } from '@openrows/ipc';
import { FileField } from './FileField';
import styles from './flow/Form.module.css';
import custom from './CustomHopFields.module.css';
import { t } from './messages/routes';

type CustomHop = Extract<HelperHop, { kind: 'custom' }>;
/** Literal command templates with an executable picker and ordered arguments. */
export function CustomHopFields({
  helper,
  set,
}: {
  helper: CustomHop;
  set: (helper: HelperHop) => void;
}): React.JSX.Element {
  const changeArgs = (args: string[]): void => {
    set({ ...helper, args });
  };
  return (
    <>
      <FileField
        label={t('program')}
        kind="program"
        value={helper.program}
        onChange={(program) => {
          set({ ...helper, program });
        }}
      />
      <ArgumentRows args={helper.args} set={changeArgs} />
      <div className={styles.actions}>
        <Button
          size="sm"
          onClick={() => {
            changeArgs([...helper.args, '']);
          }}
        >
          {t('addArgument')}
        </Button>
      </div>
      <TextField
        className={styles.row}
        label={t('readinessTimeoutSeconds')}
        type="number"
        min={1}
        max={300}
        placeholder="30"
        value={helper.readyTimeoutSeconds?.toString() ?? ''}
        onChange={(event) => {
          set({
            ...helper,
            readyTimeoutSeconds:
              event.target.value === '' ? null : Number(event.target.value),
          });
        }}
      />
      <small className={styles.hint}>
        {t('eachRowOne', {
          local_port: '{local_port}',
          remote_host: '{remote_host}',
          remote_port: '{remote_port}',
        })}
      </small>
    </>
  );
}

function ArgumentRows({
  args,
  set,
}: {
  args: string[];
  set: (args: string[]) => void;
}): React.JSX.Element {
  const move = (index: number, offset: number): void => {
    const reordered = [...args];
    const other = index + offset;
    const current = reordered[index];
    const next = reordered[other];
    if (current === undefined || next === undefined) return;
    reordered[index] = next;
    reordered[other] = current;
    set(reordered);
  };
  return (
    <>
      {args.map((arg, index) => (
        <div
          className={`${styles.fileRow ?? ''} ${styles.mono ?? ''}`}
          key={index}
        >
          <TextField
            label={t('argumentValue', { value: String(index + 1) })}
            value={arg}
            spellCheck={false}
            onChange={(event) => {
              set(
                args.map((value, row) =>
                  row === index ? event.target.value : value,
                ),
              );
            }}
          />
          <div className={custom.actions}>
            <Button
              size="sm"
              aria-label={t('moveArgumentValue', { value: String(index + 1) })}
              disabled={index === 0}
              onClick={() => {
                move(index, -1);
              }}
            >
              ↑
            </Button>
            <Button
              size="sm"
              aria-label={t('moveArgumentValueDown', {
                value: String(index + 1),
              })}
              disabled={index === args.length - 1}
              onClick={() => {
                move(index, 1);
              }}
            >
              ↓
            </Button>
            <Button
              size="sm"
              aria-label={t('removeArgumentValue', {
                value: String(index + 1),
              })}
              onClick={() => {
                set(args.filter((_, row) => row !== index));
              }}
            >
              {t('remove')}
            </Button>
          </div>
        </div>
      ))}
    </>
  );
}
