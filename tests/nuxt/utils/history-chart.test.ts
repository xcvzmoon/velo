import { describe, expect, it } from 'vitest';
import { storedHistorySeries } from '../../../app/utils/history-chart';

function update(
  sequence: string,
  elapsed: string,
  requests = '20',
): Parameters<typeof storedHistorySeries>[0][number] {
  return {
    sequence,
    interval_us: '200000',
    interval_requests: requests,
    snapshot: {
      actual_us: elapsed,
      metrics: {
        counter_overflow: false,
        iteration_rate: 100,
        assertion_failure_rate: 0.01,
      },
    },
  };
}

describe('stored history charts', () => {
  it('leaves explicit aligned gaps between retained samples after compaction', () => {
    const series = storedHistorySeries([update('2', '400000'), update('4', '800000')]);
    expect(series.seconds).toEqual([0.4, 0.6, 0.8]);
    expect(series.requests).toEqual([100, null, 100]);
    expect(series.iterations).toEqual([100, null, 100]);
    expect(series.assertionFailures).toEqual([1, null, 1]);
  });

  it('retains consecutive samples and ignores duplicate timestamps and empty intervals', () => {
    const duplicate = update('2', '200000');
    const empty = update('3', '400000');
    empty.interval_us = '0';
    const series = storedHistorySeries([
      update('1', '200000'),
      duplicate,
      empty,
      update('4', '800000'),
    ]);
    expect(series.seconds).toEqual([0.2, 0.5, 0.8]);
    expect(storedHistorySeries([update('1', '200000'), update('2', '400000')]).requests).toEqual([
      100, 100,
    ]);
  });

  it('distinguishes a measured zero from unsafe and overflowed request counts', () => {
    const overflow = update('3', '600000');
    overflow.snapshot.metrics.counter_overflow = true;
    const series = storedHistorySeries([
      update('1', '200000', '0'),
      update('2', '400000', '18446744073709551615'),
      overflow,
    ]);
    expect(series.requests).toEqual([0, null, null]);
    expect(series.iterations).toEqual([100, 100, null]);
    expect(series.assertionFailures).toEqual([1, 1, null]);
  });
});
