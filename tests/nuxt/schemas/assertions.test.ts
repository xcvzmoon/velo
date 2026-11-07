import type { Assertion } from '../../../app/types/contracts';
import * as v from 'valibot';
import { describe, expect, it } from 'vitest';
import {
  assertionEditorSchema,
  assertionInput,
  inspectionSchema,
  toAssertion,
} from '../../../app/schemas/assertions';

const assertions: Assertion[] = [
  {
    id: 'status',
    rule: {
      kind: 'status',
      allowed: [200, 204],
    },
  },
  {
    id: 'header',
    rule: {
      kind: 'header_present',
      name: 'X-Health',
    },
  },
  {
    id: 'header-value',
    rule: {
      kind: 'header_equals',
      name: 'content-type',
      value: 'application/json',
    },
  },
  {
    id: 'json',
    rule: {
      kind: 'json_present',
      pointer: '/data/a~1b',
    },
  },
  {
    id: 'json-value',
    rule: {
      kind: 'json_equals',
      pointer: '/ok',
      value: 'true',
    },
  },
  {
    id: 'contains',
    rule: {
      kind: 'text_contains',
      value: 'healthy',
    },
  },
  {
    id: 'equals',
    rule: {
      kind: 'text_equals',
      value: '',
    },
  },
];

describe('response assertion authoring', () => {
  it('round trips all supported engine assertions through the visual editor', () => {
    for (const assertion of assertions) {
      expect(toAssertion(v.parse(assertionEditorSchema, assertionInput(assertion)))).toEqual(
        assertion,
      );
    }
  });

  it('rejects invalid identifiers, status lists, headers, pointers and expectations', () => {
    for (const input of [
      {
        id: '',
        kind: 'status',
        target: '200',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'status',
        target: '200,',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'status',
        target: '600',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'header_present',
        target: 'bad name',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'header_equals',
        target: 'valid',
        expected: 'bad\nvalue',
      },
      {
        id: 'ok',
        kind: 'json_present',
        target: 'no-slash',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'json_present',
        target: '/bad~2',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'json_equals',
        target: '/ok',
        expected: 'invalid JSON',
      },
      {
        id: 'ok',
        kind: 'text_contains',
        target: '',
        expected: '',
      },
      {
        id: 'ok',
        kind: 'text_equals',
        target: '',
        expected: 'x'.repeat(4097),
      },
    ]) {
      expect(v.safeParse(assertionEditorSchema, input).success).toBe(false);
    }
  });

  it('applies the native UTF-8 byte limits to multibyte expectations and JSON pointers', () => {
    expect(
      v.safeParse(assertionEditorSchema, {
        id: 'body',
        kind: 'text_equals',
        target: '',
        expected: '界'.repeat(1400),
      }).success,
    ).toBe(false);
    expect(
      v.safeParse(assertionEditorSchema, {
        id: 'field',
        kind: 'json_present',
        target: '/' + '界'.repeat(400),
        expected: '',
      }).success,
    ).toBe(false);
  });

  it('bounds capture and retention while allowing samples to be disabled', () => {
    expect(
      v.parse(inspectionSchema, {
        body_bytes: '65536',
        sample_count: '0',
        sample_bytes: '0',
      }),
    ).toEqual({
      body_bytes: 65536,
      sample_count: 0,
      sample_bytes: 0,
    });
    for (const input of [
      {
        body_bytes: 65537,
        sample_count: 1,
        sample_bytes: 100,
      },
      {
        body_bytes: 0,
        sample_count: 1,
        sample_bytes: 100,
      },
      {
        body_bytes: 100,
        sample_count: 33,
        sample_bytes: 100,
      },
      {
        body_bytes: 100,
        sample_count: 1,
        sample_bytes: 32769,
      },
    ]) {
      expect(v.safeParse(inspectionSchema, input).success).toBe(false);
    }
  });
});
