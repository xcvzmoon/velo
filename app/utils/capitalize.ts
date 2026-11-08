/**
 * Uppercases the first character while preserving the remaining text.
 * @param input - Text to capitalize; may be empty.
 * @returns The capitalized text, or an empty string for empty input.
 * @example
 * ```ts
 * capitalize('request') === 'Request'
 * capitalize('') === ''
 * ```
 */
export function capitalize(input: string): string {
  return input[0] ? input[0].toUpperCase() + input.slice(1) : '';
}
