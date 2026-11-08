import type { RunSnapshot } from '~/types/contracts';

export type OverviewMetric = {
  label: string;
  value: string;
  unit: string;
  detail: string;
  icon: string;
};

export type OverviewTimeline = {
  seconds: number[];
  requests: (number | null)[];
  latency: (number | null)[];
};

/**
 * Formats full-run dashboard measurements without rounding integer counters.
 * @param snapshot - A native snapshot, or null when no run is available.
 * @returns Four display metrics. Missing observations use an em dash; zero remains zero.
 * Latency is converted from microseconds to milliseconds, and failure ratios to percentages.
 * @throws SyntaxError if the native request counter is not an integer string.
 * @example
 * ```ts
 * const metrics = overviewMetrics(null)
 * metrics[0]?.value
 * ```
 */
export function overviewMetrics(snapshot: RunSnapshot | null): OverviewMetric[] {
  const metrics = snapshot?.metrics;
  const rate = metrics?.request_rate ?? null;
  const latency = metrics?.response_latency.p95_us ?? null;
  const failureRate = metrics?.http_failure_rate ?? null;
  const formatter = new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 });

  return [
    {
      label: 'Requests started',
      value: metrics ? BigInt(metrics.counters.started).toLocaleString('en-US') : '—',
      unit: '',
      detail: 'Admitted HTTP requests',
      icon: 'i-hugeicons-sent',
    },
    {
      label: 'Throughput',
      value: rate === null ? '—' : formatter.format(rate),
      unit: rate === null ? '' : 'req/s',
      detail: 'Non-cancelled terminal requests',
      icon: 'i-hugeicons-dashboard-speed-01',
    },
    {
      label: 'Response p95',
      value: latency === null ? '—' : formatter.format(latency / 1000),
      unit: latency === null ? '' : 'ms',
      detail: 'Completed HTTP responses',
      icon: 'i-hugeicons-timer-02',
    },
    {
      label: 'HTTP failures',
      value: failureRate === null ? '—' : formatter.format(failureRate * 100),
      unit: failureRate === null ? '' : '%',
      detail: 'Cancelled requests excluded',
      icon: 'i-hugeicons-alert-circle',
    },
  ];
}
