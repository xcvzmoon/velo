import * as v from 'valibot';

const errorSchema = v.object({
  code: v.string(),
  message: v.string(),
  field: v.nullable(v.string()),
});

/**
 * Formats a native authoring failure without exposing unstructured infrastructure errors.
 * @param failure - Rejected native command value.
 * @returns A stable error code, safe message, and affected field when available.
 * @example
 * ```ts
 * formatAuthoringError({ code: 'VARIABLE_UNRESOLVED', message: 'Define the variable.', field: 'url' })
 * ```
 */
export function formatAuthoringError(failure: Parameters<typeof v.safeParse>[1]): string {
  const result = v.safeParse(errorSchema, failure);
  if (!result.success) {
    return 'AUTHORING_FAILED: The operation could not complete.';
  }
  const { code, message, field } = result.output;
  return `${code}${field ? ` (${field})` : ''}: ${message}`;
}
