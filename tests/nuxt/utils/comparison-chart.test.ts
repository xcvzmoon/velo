import type { MetricDelta } from '../../../app/types/contracts';
import { describe, expect, it } from 'vitest';
import { comparisonChart } from '../../../app/utils/comparison-chart';

function delta(baseline: number | null, candidate: number | null): MetricDelta {
  return {
    metric: 'request_rate',
    baseline,
    candidate,
    absolute: null,
    percentage: null,
  };
}

describe('baseline comparison charts', () => {
  it('uses a shared zero-based scale within each metric', () => {
    const chart = comparisonChart(delta(100, 80));
    expect(chart.maximum).toBe(100);
    expect(chart.rows).toEqual([
      {
        label: 'Baseline',
        value: 100,
        width: 100,
      },
      {
        label: 'Candidate',
        value: 80,
        width: 80,
      },
    ]);
    expect(comparisonChart(delta(100, 200)).rows[0]?.width).toBe(50);
  });

  it('distinguishes measured zero from unavailable observations', () => {
    expect(comparisonChart(delta(0, 0)).rows.map(({ width }) => width)).toEqual([0, 0]);
    expect(comparisonChart(delta(null, 0)).rows.map(({ width }) => width)).toEqual([null, 0]);
    expect(comparisonChart(delta(null, null)).maximum).toBeNull();
    expect(comparisonChart(delta(null, null)).rows.every(({ width }) => width === null)).toBe(true);
  });

  it('retains unrounded values and avoids invalid geometry at numeric extremes', () => {
    const chart = comparisonChart(delta(0.123456789, Number.MAX_VALUE));
    expect(chart.rows[0]?.value).toBe(0.123456789);
    for (const { width } of chart.rows) {
      expect(width).not.toBeNull();
      expect(Number.isFinite(width)).toBe(true);
      expect(width).toBeGreaterThanOrEqual(0);
      expect(width).toBeLessThanOrEqual(100);
    }
    expect(comparisonChart(delta(Number.NaN, Number.POSITIVE_INFINITY)).maximum).toBeNull();
    expect(comparisonChart(delta(-1, 10)).rows[0]?.width).toBeNull();
  });
});
