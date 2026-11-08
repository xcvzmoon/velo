import { describe, expect, it } from 'vitest';
import { overviewMetrics } from '../../../app/utils/overview';
import { runFixture } from '../../browser/fixtures/run';

describe('overview measurements', () => {
  it('does not invent zero measurements for an empty workspace', () => {
    const metrics = overviewMetrics(null);
    expect(metrics).toHaveLength(4);
    expect(metrics.every(({ value, unit }) => value === '—' && unit === '')).toBe(true);
  });

  it('retains integer precision beyond the JavaScript safe integer range', () => {
    expect(overviewMetrics(runFixture)[0]?.value).toBe('9,007,199,254,740,993');
  });

  it('converts response latency and failure ratios to the displayed units', () => {
    const metrics = overviewMetrics({
      ...runFixture,
      metrics: {
        ...runFixture.metrics,
        response_latency: {
          ...runFixture.metrics.response_latency,
          p95_us: 1250,
        },
        http_failure_rate: 0.125,
      },
    });
    expect(metrics[2]).toMatchObject({
      value: '1.25',
      unit: 'ms',
    });
    expect(metrics[3]).toMatchObject({
      value: '12.5',
      unit: '%',
    });
  });

  it('keeps measured zero rates distinct from unavailable latency', () => {
    const metrics = overviewMetrics({
      ...runFixture,
      metrics: {
        ...runFixture.metrics,
        request_rate: 0,
        http_failure_rate: 0,
        response_latency: {
          ...runFixture.metrics.response_latency,
          p95_us: null,
        },
      },
    });
    expect(metrics[1]).toMatchObject({
      value: '0',
      unit: 'req/s',
    });
    expect(metrics[2]).toMatchObject({
      value: '—',
      unit: '',
    });
    expect(metrics[3]).toMatchObject({
      value: '0',
      unit: '%',
    });
  });
});
