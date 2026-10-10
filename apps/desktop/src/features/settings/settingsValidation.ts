// SPDX-License-Identifier: Apache-2.0
import schemaText from '../../../src-tauri/settings.schema.json?raw';
import { validTheme } from '@openrows/ui';
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
    fail(path, 'unsupported version');
  if (rule.enum && !rule.enum.includes(value)) fail(path, 'invalid choice');
  if (rule.type === 'integer' && !Number.isSafeInteger(value))
    fail(path, 'must be an integer');
  validateRange(value, rule, path);
}

function validateRange(value: unknown, rule: Schema, path: string): void {
  if (typeof value === 'number') {
    if (
      !Number.isFinite(value) ||
      value < (rule.minimum ?? -Infinity) ||
      value > (rule.maximum ?? Infinity)
    )
      fail(path, 'number outside the allowed range');
  }
  if (typeof value === 'string') {
    if (
      value.length < (rule.minLength ?? 0) ||
      value.length > (rule.maxLength ?? Infinity)
    )
      fail(path, 'invalid length');
    if (rule.pattern && !new RegExp(rule.pattern).test(value))
      fail(path, 'invalid format');
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
    } else fail(`${path}.${key}`, 'unknown key');
  }
}

function validate(value: unknown, input: Schema, path: string): void {
  const rule = input.$ref
    ? definitions[input.$ref.split('/').at(-1) ?? '']
    : input;
  if (!rule) return fail(path, 'unknown schema');
  if (rule.oneOf) {
    const matches = rule.oneOf.filter((choice) => {
      try {
        validate(value, choice, path);
        return true;
      } catch {
        return false;
      }
    });
    if (matches.length !== 1) fail(path, 'invalid value');
    return;
  }
  const actual =
    value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (rule.type && rule.type !== 'integer' && rule.type !== actual)
    fail(path, `must be ${rule.type}`);
  scalar(value, rule, path);
  if (Array.isArray(value)) {
    if (value.length > (rule.maxItems ?? Infinity))
      fail(path, 'too many items');
    value.forEach((item, index) => {
      if (rule.items) validate(item, rule.items, `${path}[${String(index)}]`);
    });
  } else if (actual === 'object')
    object(value as Record<string, unknown>, rule, path);
}

/** Validate portable settings strictly, including nested unknown and unsafe keys. */
export function parsePortableSettings(text: string): Record<string, unknown> {
  if (text.length > 1024 * 1024) fail('Settings', 'larger than 1 MiB');
  const value: unknown = JSON.parse(text);
  validate(value, schema, 'Settings');
  const themes = (value as { appearance?: { customThemes?: unknown[] } })
    .appearance?.customThemes;
  if (themes?.some((theme) => !validTheme(theme)))
    fail('Settings.appearance.customThemes', 'theme fails contrast validation');
  return value as Record<string, unknown>;
}
