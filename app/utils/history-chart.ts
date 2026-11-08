import type { MetricsSnapshot, RunSnapshot, RunUpdate } from '../types/contracts';

type HistoryChartUpdate = Pick<RunUpdate, 'sequence' | 'interval_us' | 'interval_requests'> & {
  snapshot: Pick<RunSnapshot, 'actual_us'> & {
    metrics: Pick<
      MetricsSnapshot,
      'counter_overflow' | 'iteration_rate' | 'assertion_failure_rate'
    >;
  };
};

type HistoryChartSeries = {
  seconds: number[];
  requests: (number | null)[];
  iterations: (number | null)[];
  assertionFailures: (number | null)[];
};

/**
 * Builds stored-run chart series without connecting across missing aggregate intervals.
 * @param updates - Ordered native history samples with decimal counters and elapsed microseconds.
 * @returns Aligned series with null gaps for sequence discontinuities. Invalid timing is omitted;
 * overflowed or unsafe counters leave request rates unavailable. Input measurements are unchanged.
 * @example
 * ```typescript
 * const series = storedHistorySeries(entry.points)
 * // Bind series.seconds and series.requests to a stored request-rate chart.
 * ```
 */
export function storedHistorySeries(updates: readonly HistoryChartUpdate[]): HistoryChartSeries {
  const result: HistoryChartSeries = {
    seconds: [],
    requests: [],
    iterations: [],
    assertionFailures: [],
  };
  let previousElapsed = -1;
  let previousSequence: bigint | null = null;
  for (const { sequence, interval_us, interval_requests, snapshot } of updates) {
    const elapsed = Number(snapshot.actual_us);
    const interval = Number(interval_us);
    if (
      !Number.isSafeInteger(elapsed) ||
      elapsed <= previousElapsed ||
      !Number.isSafeInteger(interval) ||
      interval <= 0
    ) {
      continue;
    }
    const currentSequence = BigInt(sequence);
    if (previousSequence !== null && currentSequence !== previousSequence + 1n) {
      result.seconds.push((previousElapsed + (elapsed - previousElapsed) / 2) / 1e6);
      result.requests.push(null);
      result.iterations.push(null);
      result.assertionFailures.push(null);
    }
    const requests = Number(interval_requests);
    const { counter_overflow, iteration_rate, assertion_failure_rate } = snapshot.metrics;
    result.seconds.push(elapsed / 1e6);
    result.requests.push(
      !counter_overflow && Number.isSafeInteger(requests) && requests >= 0
        ? (requests / interval) * 1e6
        : null,
    );
    result.iterations.push(
      !counter_overflow && Number.isFinite(iteration_rate) && iteration_rate >= 0
        ? iteration_rate
        : null,
    );
    result.assertionFailures.push(
      !counter_overflow &&
        assertion_failure_rate !== null &&
        Number.isFinite(assertion_failure_rate) &&
        assertion_failure_rate >= 0 &&
        assertion_failure_rate <= 1
        ? assertion_failure_rate * 100
        : null,
    );
    previousElapsed = elapsed;
    previousSequence = currentSequence;
  }
  return result;
}
