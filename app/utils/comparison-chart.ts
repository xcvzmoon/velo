import type { MetricDelta, RegressionMetric } from '../types/contracts';

type ComparisonChartRow = {
  label: 'Baseline' | 'Candidate';
  value: number | null;
  width: number | null;
};

type ComparisonChart = {
  metric: RegressionMetric;
  label: string;
  maximum: number | null;
  rows: ComparisonChartRow[];
};

const metricLabels: Record<RegressionMetric, string> = {
  response_p50_ms: 'Response p50 (ms)',
  response_p95_ms: 'Response p95 (ms)',
  response_p99_ms: 'Response p99 (ms)',
  request_rate: 'Throughput (requests/s)',
  http_failure_percent: 'HTTP failure (%)',
  assertion_failure_percent: 'Assertion failure (%)',
};

/**
 * Builds separately scaled baseline/candidate bars from retained evaluated measurements.
 * @param delta - Unrounded values for one metric; missing or invalid values remain unavailable.
 * @returns Two labeled rows with widths from 0 to 100 and a common zero-based scale.
 * An all-zero population has zero-width bars; an absent population has no bar.
 * @example
 * ```typescript
 * comparisonChart({ metric: 'request_rate', baseline: 100, candidate: 80,
 *   absolute: -20, percentage: -20 }).rows[1]?.width // 80
 * ```
 */
export function comparisonChart(delta: MetricDelta): ComparisonChart {
  const baseline = validMeasurement(delta.baseline);
  const candidate = validMeasurement(delta.candidate);
  const maximum =
    baseline === null && candidate === null ? null : Math.max(baseline ?? 0, candidate ?? 0);
  return {
    metric: delta.metric,
    label: metricLabels[delta.metric],
    maximum,
    rows: [
      {
        label: 'Baseline',
        value: baseline,
        width: barWidth(baseline, maximum),
      },
      {
        label: 'Candidate',
        value: candidate,
        width: barWidth(candidate, maximum),
      },
    ],
  };
}

function validMeasurement(value: number | null): number | null {
  return value !== null && Number.isFinite(value) && value >= 0 ? value : null;
}

function barWidth(value: number | null, maximum: number | null): number | null {
  if (value === null || maximum === null) {
    return null;
  }
  return maximum === 0 ? 0 : (value / maximum) * 100;
}
