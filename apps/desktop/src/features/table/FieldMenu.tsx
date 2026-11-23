// SPDX-License-Identifier: Apache-2.0
import { Fragment, useId } from 'react';
import { SidebarMenu } from '../../shell/SidebarMenu';
import { DEFAULT_VALUE, NULL_VALUE, textValue } from './cellValue';
import type { CellValue } from './cellValue';
import type { FieldActions } from './fieldActions';
import { t } from './messages/initial';

type Item = {
  label: string;
  value?: CellValue | undefined;
  file?: boolean;
  inspect?: boolean;
};

/** Show or hide the field's exact stored value under it. */
export type FieldInspect = { open: boolean; toggle: () => void };

/** TablePlus's value menu: full value, markers, JSON formatting, file contents. */
function items(actions: FieldActions, inspect?: FieldInspect): Item[][] {
  const full: Item[][] = inspect
    ? [
        [
          {
            label: inspect.open ? t('hideFullValue') : t('fullValue'),
            inspect: true,
          },
        ],
      ]
    : [];
  return [
    ...full,
    [
      { label: t('null'), value: actions.null ? NULL_VALUE : undefined },
      {
        label: t('default'),
        value: actions.default ? DEFAULT_VALUE : undefined,
      },
      { label: t('empty'), value: actions.empty ? textValue('') : undefined },
    ],
    [
      { label: t('pretty'), value: json(actions.pretty) },
      { label: t('minify'), value: json(actions.minify) },
    ],
    [{ label: t('addFile'), file: actions.file !== null }],
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
  /** Present when the row came from a stored result: offers the full value. */
  inspect?: FieldInspect | undefined;
}): React.JSX.Element {
  const id = useId();
  const choose = (item: Item): void => {
    props.onClose();
    if (item.inspect) props.inspect?.toggle();
    else if (item.value) props.onValue(item.value);
    else if (item.file) props.onFile();
  };
  return (
    <SidebarMenu
      id={id}
      label={t('valueName', { name: props.name })}
      opener={props.opener}
      onClose={props.onClose}
    >
      {items(props.actions, props.inspect).map((group, index) => (
        <Fragment key={index}>
          {index > 0 && <hr />}
          {group.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              disabled={!item.value && !item.file && !item.inspect}
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
