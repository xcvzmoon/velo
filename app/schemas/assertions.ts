import type { Assertion } from '~/types/contracts';
import * as v from 'valibot';

export const assertionEditorSchema = v.pipe(
  v.object({
    id: v.pipe(
      v.string(),
      v.regex(/^[A-Za-z0-9_-]{1,64}$/, 'Use 1–64 letters, digits, underscores or hyphens.'),
    ),
    kind: v.picklist([
      'status',
      'header_present',
      'header_equals',
      'json_present',
      'json_equals',
      'text_contains',
      'text_equals',
    ]),
    target: v.pipe(v.string(), v.maxBytes(1024, 'Use at most 1,024 UTF-8 bytes.')),
    expected: v.pipe(v.string(), v.maxBytes(4096, 'Use at most 4,096 UTF-8 bytes.')),
  }),
  v.forward(
    v.check(({ kind, target }) => {
      if (kind === 'status') {
        const statuses = target.split(',');
        return (
          statuses.length <= 100 && statuses.every((status) => /^\s*[1-5]\d{2}\s*$/.test(status))
        );
      }
      if (kind === 'header_present' || kind === 'header_equals') {
        return target.length <= 256 && /^[!#$%&'*+.^_`|~\w-]+$/.test(target);
      }
      if (kind === 'json_present' || kind === 'json_equals') {
        return (target === '' || target.startsWith('/')) && !/~(?:[^01]|$)/.test(target);
      }
      return true;
    }, 'Enter allowed statuses, a valid header name, or an RFC 6901 JSON pointer.'),
    ['target'],
  ),
  v.forward(
    v.check(({ kind, expected }) => {
      if (kind === 'json_equals') {
        return v.is(v.pipe(v.string(), v.parseJson()), expected);
      }
      if (kind === 'text_contains') {
        return expected.length > 0;
      }
      return kind !== 'header_equals' || /^[\t\x20-\x7E\u0080-\u00FF]*$/.test(expected);
    }, 'Enter valid JSON for JSON equality, a nonempty text literal, or a valid header value.'),
    ['expected'],
  ),
);

export const inspectionSchema = v.object({
  body_bytes: v.pipe(
    v.union([v.string(), v.number()]),
    v.toNumber(),
    v.integer(),
    v.minValue(1),
    v.maxValue(65_536),
  ),
  sample_count: v.pipe(
    v.union([v.string(), v.number()]),
    v.toNumber(),
    v.integer(),
    v.minValue(0),
    v.maxValue(32),
  ),
  sample_bytes: v.pipe(
    v.union([v.string(), v.number()]),
    v.toNumber(),
    v.integer(),
    v.minValue(0),
    v.maxValue(32_768),
  ),
});

/**
 * Converts a validated editor row into an engine assertion.
 * @param input - Row validated by assertionEditorSchema.
 * @returns The named assertion with its typed condition.
 * @example
 * ```ts
 * toAssertion({ id: 'ok', kind: 'status', target: '200,204', expected: '' })
 * ```
 */
export function toAssertion(input: v.InferOutput<typeof assertionEditorSchema>): Assertion {
  const { id, kind, target, expected } = input;
  switch (kind) {
    case 'status':
      return {
        id,
        rule: {
          kind,
          allowed: target.split(',').map((status) => Number(status.trim())),
        },
      };
    case 'header_present':
      return {
        id,
        rule: {
          kind,
          name: target,
        },
      };
    case 'header_equals':
      return {
        id,
        rule: {
          kind,
          name: target,
          value: expected,
        },
      };
    case 'json_present':
      return {
        id,
        rule: {
          kind,
          pointer: target,
        },
      };
    case 'json_equals':
      return {
        id,
        rule: {
          kind,
          pointer: target,
          value: expected,
        },
      };
    case 'text_contains':
    case 'text_equals':
      return {
        id,
        rule: {
          kind,
          value: expected,
        },
      };
  }
  return kind satisfies never;
}

/**
 * Restores an engine assertion into its editable fields.
 * @param assertion - Saved or imported assertion.
 * @returns A row for the Formisch assertion editor.
 * @example
 * ```ts
 * assertionInput({ id: 'ok', rule: { kind: 'status', allowed: [200] } })
 * ```
 */
export function assertionInput({
  id,
  rule,
}: Assertion): v.InferInput<typeof assertionEditorSchema> {
  switch (rule.kind) {
    case 'status':
      return {
        id,
        kind: rule.kind,
        target: rule.allowed.join(','),
        expected: '',
      };
    case 'header_present':
      return {
        id,
        kind: rule.kind,
        target: rule.name,
        expected: '',
      };
    case 'header_equals':
      return {
        id,
        kind: rule.kind,
        target: rule.name,
        expected: rule.value,
      };
    case 'json_present':
      return {
        id,
        kind: rule.kind,
        target: rule.pointer,
        expected: '',
      };
    case 'json_equals':
      return {
        id,
        kind: rule.kind,
        target: rule.pointer,
        expected: rule.value,
      };
    case 'text_contains':
    case 'text_equals':
      return {
        id,
        kind: rule.kind,
        target: '',
        expected: rule.value,
      };
  }
  return rule satisfies never;
}
