// SPDX-License-Identifier: Apache-2.0
// Visible text belongs in a message catalog (docs/07 §10): JSX text, text
// attributes and label-like object properties must not be string literals.

const LETTER = /\p{L}/u;

/** Elements whose text is a code sample, a stylesheet or a script. */
const CODE_ELEMENTS = new Set(['code', 'pre', 'style', 'script']);

const TEXT_ATTRIBUTES = new Set([
  'action',
  'alt',
  'aria-description',
  'aria-label',
  'aria-placeholder',
  'aria-roledescription',
  'aria-valuetext',
  'description',
  'detail',
  'error',
  'heading',
  'hint',
  'idle',
  'label',
  'legend',
  'menuLabel',
  'message',
  'noun',
  'placeholder',
  'stageLabel',
  'summary',
  'term',
  'title',
  'tooltip',
]);

const TEXT_PROPERTIES = new Set([
  'description',
  'detail',
  'heading',
  'hint',
  'label',
  'message',
  'note',
  'placeholder',
  'reason',
  'summary',
  'title',
  'tooltip',
]);

function hasText(node, allowed) {
  if (node.type === 'Literal') {
    return (
      typeof node.value === 'string' &&
      LETTER.test(node.value) &&
      !allowed.has(node.value)
    );
  }
  if (node.type === 'TemplateLiteral') {
    return node.quasis.some((quasi) => LETTER.test(quasi.value.cooked ?? ''));
  }
  return false;
}

/** The expressions that decide the value: both branches, never a condition. */
function valueLeaves(node) {
  if (node.type === 'ConditionalExpression') {
    return [...valueLeaves(node.consequent), ...valueLeaves(node.alternate)];
  }
  if (node.type === 'LogicalExpression') {
    const right = valueLeaves(node.right);
    return node.operator === '&&'
      ? right
      : [...valueLeaves(node.left), ...right];
  }
  if (node.type === 'BinaryExpression' && node.operator === '+') {
    return [...valueLeaves(node.left), ...valueLeaves(node.right)];
  }
  if (node.type === 'TSAsExpression' || node.type === 'TSSatisfiesExpression') {
    return valueLeaves(node.expression);
  }
  return [node];
}

function isInCodeElement(node) {
  const opening = node.parent.openingElement;
  return opening?.name.type === 'JSXIdentifier'
    ? CODE_ELEMENTS.has(opening.name.name)
    : false;
}

function propertyName(node) {
  if (node.computed) return undefined;
  return node.key.type === 'Identifier' ? node.key.name : node.key.value;
}

/** @type {import('eslint').Rule.RuleModule} */
export const noHardcodedUiText = {
  meta: {
    type: 'problem',
    docs: { description: 'Require visible text to come from a catalog.' },
    messages: {
      literal:
        'Move this text to a message catalog and read it with t() (docs/07 §10).',
    },
    schema: [
      {
        type: 'object',
        properties: {
          allow: { type: 'array', items: { type: 'string' } },
        },
        additionalProperties: false,
      },
    ],
  },
  create(context) {
    const allowed = new Set(context.options[0]?.allow ?? []);
    const reportLeaves = (expression) => {
      for (const leaf of valueLeaves(expression)) {
        if (hasText(leaf, allowed)) {
          context.report({ node: leaf, messageId: 'literal' });
        }
      }
    };
    return {
      JSXText(node) {
        const text = node.value.trim();
        if (!LETTER.test(text) || allowed.has(text)) return;
        if (isInCodeElement(node)) return;
        context.report({ node, messageId: 'literal' });
      },
      JSXExpressionContainer(node) {
        if (node.parent.type === 'JSXAttribute') return;
        if (node.expression.type === 'JSXEmptyExpression') return;
        if (isInCodeElement(node)) return;
        reportLeaves(node.expression);
      },
      JSXAttribute(node) {
        if (!node.value || !TEXT_ATTRIBUTES.has(String(node.name.name))) return;
        reportLeaves(
          node.value.type === 'JSXExpressionContainer'
            ? node.value.expression
            : node.value,
        );
      },
      Property(node) {
        if (TEXT_PROPERTIES.has(propertyName(node))) reportLeaves(node.value);
      },
    };
  },
};
