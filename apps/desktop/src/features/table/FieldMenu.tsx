// SPDX-License-Identifier: Apache-2.0
import { Fragment, useId } from 'react';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import type { FieldActions } from './fieldActions';

type Item = { label: string; value?: CellValue | undefined; file?: boolean };

/** TablePlus's value menu: markers, JSON formatting, then file contents. */
function items(actions: FieldActions): Item[][] {
  return [
    [
      { label: 'NULL', value: actions.null ? NULL_VALUE : undefined },
      { label: 'DEFAULT', value: actions.default ? DEFAULT_VALUE : undefined },
      { label: 'EMPTY', value: actions.empty ? textValue('') : undefined },
    ],
    [
      { label: 'PRETTY', value: json(actions.pretty) },
      { label: 'MINIFY', value: json(actions.minify) },
    ],
    [{ label: 'Add a file…', file: actions.file !== null }],
  ];
}

function json(text: string | null): CellValue | undefined {
  return text === null ? undefined : textValue(text);
}

/** The chevron menu of one inspector field; choosing an item only stages. */
export function FieldMenu(props: {
  name: string;
  actions: FieldActions;
  opener: React.RefObject<HTMLButtonElement | null>;
  onValue: (value: CellValue) => void;
  onFile: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const id = useId();
  const choose = (item: Item): void => {
    props.onClose();
    if (item.value) props.onValue(item.value);
    else if (item.file) props.onFile();
  };
  return (
    <SidebarMenu
      id={id}
      label={`Value of ${props.name}`}
      opener={props.opener}
      onClose={props.onClose}
    >
      {items(props.actions).map((group, index) => (
        <Fragment key={index}>
          {index > 0 && <hr />}
          {group.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              disabled={!item.value && !item.file}
              onClick={() => {
                choose(item);
              }}
            >
              {item.label}
            </button>
          ))}
        </Fragment>
      ))}
    </SidebarMenu>
  );
}
