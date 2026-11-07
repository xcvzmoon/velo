import { describe, expect, it } from 'vitest';
import { formatAuthoringError } from '../../../app/utils/authoring';

describe('native authoring errors', () => {
  it('identifies the field without exposing unstructured rejection data', () => {
    expect(
      formatAuthoringError({
        code: 'VARIABLE_UNRESOLVED',
        message: 'Define the referenced variable.',
        field: 'headers.0.value',
      }),
    ).toBe('VARIABLE_UNRESOLVED (headers.0.value): Define the referenced variable.');
    expect(formatAuthoringError(new Error('PRIVATE_INFRASTRUCTURE_DATA'))).toBe(
      'AUTHORING_FAILED: The operation could not complete.',
    );
  });
});
