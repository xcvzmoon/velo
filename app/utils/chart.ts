/**
 * Detects observations that would disappear when a chart draws only line segments.
 * @param values - Ordered series values; null and undefined mark gaps.
 * @param first - First visible sample index, inclusive.
 * @param last - Last visible sample index, inclusive.
 * @returns True when a visible observation has no adjacent visible observation to connect to.
 * @example
 * ```typescript
 * hasIsolatedChartPoints([20, null, 40], 0, 2) // true
 * ```
 */
export function hasIsolatedChartPoints(
  values: readonly (number | null | undefined)[],
  first: number,
  last: number,
): boolean {
  for (let index = Math.max(0, first); index <= Math.min(last, values.length - 1); index++) {
    const value = values[index];
    const previous = index <= first ? null : values[index - 1];
    const next = index >= last ? null : values[index + 1];
    if (
      value !== null &&
      value !== undefined &&
      (previous ?? null) === null &&
      (next ?? null) === null
    ) {
      return true;
    }
  }
  return false;
}
