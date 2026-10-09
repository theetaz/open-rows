// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import type { HelperHop } from '@openrows/ipc';
import { FileField } from './FileField';
import styles from './ConnectionSheet.module.css';
import custom from './CustomHopFields.module.css';

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
        label="Program"
        kind="program"
        value={helper.program}
        onChange={(program) => {
          set({ ...helper, program });
        }}
      />
      <ArgumentRows args={helper.args} set={changeArgs} />
      <Button
        onClick={() => {
          changeArgs([...helper.args, '']);
        }}
      >
        Add argument
      </Button>
      <TextField
        className={styles.row}
        label="Readiness timeout (seconds)"
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
      <small className={custom.help}>
        Each row is one literal argument, including spaces. Use {'{local_port}'}{' '}
        to bind 127.0.0.1 on the allocated port; {'{remote_host}'} and{' '}
        {'{remote_port}'} come from the database host and port. No shell
        expansion. Do not put secrets in arguments. Test or connect asks you to
        Trust and run on this machine; changing the program or arguments
        requires approval again.
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
        <div className={custom.argumentRow} key={index}>
          <TextField
            className={custom.argument}
            label={`Argument ${String(index + 1)}`}
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
              aria-label={`Move argument ${String(index + 1)} up`}
              disabled={index === 0}
              onClick={() => {
                move(index, -1);
              }}
            >
              ↑
            </Button>
            <Button
              aria-label={`Move argument ${String(index + 1)} down`}
              disabled={index === args.length - 1}
              onClick={() => {
                move(index, 1);
              }}
            >
              ↓
            </Button>
            <Button
              aria-label={`Remove argument ${String(index + 1)}`}
              onClick={() => {
                set(args.filter((_, row) => row !== index));
              }}
            >
              Remove
            </Button>
          </div>
        </div>
      ))}
    </>
  );
}
