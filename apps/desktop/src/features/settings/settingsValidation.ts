// SPDX-License-Identifier: Apache-2.0
import schemaText from '../../../src-tauri/settings.schema.json?raw';
import {
  copyConflict,
  parseCopySettings,
} from '../../results/copy/copySettings';
import { validTheme } from '@openrows/ui';
import { t } from './messages/validation';
const schema = JSON.parse(schemaText) as {
  definitions: Record<string, Schema>;
} & Schema;

type Schema = {
  $ref?: string;
  type?: string;
  const?: unknown;
  enum?: unknown[];
  required?: string[];
  properties?: Record<string, Schema>;
  propertyNames?: Schema;
  additionalProperties?: Schema | boolean;
  oneOf?: Schema[];
  items?: Schema;
  pattern?: string;
  minLength?: number;
  maxLength?: number;
  minimum?: number;
  maximum?: number;
  maxItems?: number;
};
const definitions = schema.definitions;
const fail = (path: string, reason: string): never => {
  throw new Error(`${path}: ${reason}`);
};

function scalar(value: unknown, rule: Schema, path: string): void {
  if ('const' in rule && value !== rule.const)
    fail(path, t('unsupportedVersion'));
  if (rule.enum && !rule.enum.includes(value)) fail(path, t('invalidChoice'));
  if (rule.type === 'integer' && !Number.isSafeInteger(value))
    fail(path, t('mustInteger'));
  validateRange(value, rule, path);
}

function validateRange(value: unknown, rule: Schema, path: string): void {
  if (typeof value === 'number') {
    if (
      !Number.isFinite(value) ||
      value < (rule.minimum ?? -Infinity) ||
      value > (rule.maximum ?? Infinity)
    )
      fail(path, t('numberOutsideAllowed'));
  }
  if (typeof value === 'string') {
    if (
      value.length < (rule.minLength ?? 0) ||
      value.length > (rule.maxLength ?? Infinity)
    )
      fail(path, t('invalidLength'));
    if (rule.pattern && !new RegExp(rule.pattern).test(value))
      fail(path, t('invalidFormat'));
  }
}

function object(
  value: Record<string, unknown>,
  rule: Schema,
  path: string,
): void {
  for (const key of rule.required ?? [])
    if (!Object.hasOwn(value, key)) fail(`${path}.${key}`, 'required');
  for (const [key, field] of Object.entries(value)) {
    const property =
      rule.properties && Object.hasOwn(rule.properties, key)
        ? rule.properties[key]
        : undefined;
    if (property) validate(field, property, `${path}.${key}`);
    else if (typeof rule.additionalProperties === 'object') {
      if (rule.propertyNames)
        validate(key, rule.propertyNames, `${path}.${key}`);
      validate(field, rule.additionalProperties, `${path}.${key}`);
    } else fail(`${path}.${key}`, t('unknownKey'));
  }
}

function validate(value: unknown, input: Schema, path: string): void {
  const rule = input.$ref
    ? definitions[input.$ref.split('/').at(-1) ?? '']
    : input;
  if (!rule) return fail(path, t('unknownSchema'));
  if (rule.oneOf) {
    const matches = rule.oneOf.filter((choice) => {
      try {
        validate(value, choice, path);
        return true;
      } catch {
        return false;
      }
    });
    if (matches.length !== 1) fail(path, t('invalidValue'));
    return;
  }
  const actual =
    value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (rule.type && rule.type !== 'integer' && rule.type !== actual)
    fail(path, t('mustType', { type: rule.type }));
  scalar(value, rule, path);
  if (Array.isArray(value)) {
    if (value.length > (rule.maxItems ?? Infinity))
      fail(path, t('tooManyItems'));
    value.forEach((item, index) => {
      if (rule.items) validate(item, rule.items, `${path}[${String(index)}]`);
    });
  } else if (actual === 'object')
    object(value as Record<string, unknown>, rule, path);
}

/** Validate portable settings strictly, including nested unknown and unsafe keys. */
export function parsePortableSettings(text: string): Record<string, unknown> {
  if (text.length > 1024 * 1024) fail(t('settings'), t('largerThan1'));
  const value: unknown = JSON.parse(text);
  validate(value, schema, t('settings'));
  const copy = copyConflict(
    parseCopySettings((value as { copy?: unknown }).copy),
  );
  if (copy) fail('Settings.copy', copy);
  const themes = (value as { appearance?: { customThemes?: unknown[] } })
    .appearance?.customThemes;
  if (themes?.some((theme) => !validTheme(theme)))
    fail('Settings.appearance.customThemes', t('themeFailsContrast'));
  return value as Record<string, unknown>;
}
