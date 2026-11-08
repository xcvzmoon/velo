import type { HostTelemetry } from '../../../app/types/contracts';
import type { GeneratorPoint } from '../../../app/utils/generator-health';
import { describe, expect, it } from 'vitest';
import {
  generatorObservations,
  generatorResourcePoints,
  isTelemetryAvailable,
  resourcePressure,
} from '../../../app/utils/generator-health';

const telemetry: HostTelemetry = {
  status: 'available',
  sampled_at_us: '1000000',
  interval_us: '1000000',
  logical_cpus: 8,
  process_cpu_percent: 12.5,
  process_memory_bytes: '104857600',
  system_cpu_percent: 40,
  system_available_memory_bytes: '8589934592',
  system_total_memory_bytes: '17179869184',
};

function point(seconds: number, sample: HostTelemetry | null = telemetry): GeneratorPoint {
  return {
    seconds,
    telemetry: sample,
    capacityDrops: '0',
    missedDeadlines: '0',
    vus: 8,
    desiredVus: 8,
    retiringVus: 0,
  };
}

describe('generator evidence', () => {
  it('keeps missing, partial, stale, unsupported and future measurements unknown', () => {
    expect(resourcePressure(null, 1).pressure).toBeNull();
    expect(
      resourcePressure(
        {
          ...telemetry,
          status: 'unsupported',
        },
        1,
      ).pressure,
    ).toBeNull();
    expect(
      resourcePressure(
        {
          ...telemetry,
          status: 'unavailable',
        },
        1,
      ).pressure,
    ).toBeNull();
    expect(
      resourcePressure(
        {
          ...telemetry,
          status: 'warming_up',
        },
        1,
      ).pressure,
    ).toBeNull();
    expect(
      resourcePressure(
        {
          ...telemetry,
          process_cpu_percent: null,
        },
        1,
      ),
    ).toEqual({
      evidence: ['Some resource measurements unavailable'],
      pressure: null,
    });
    expect(isTelemetryAvailable(telemetry, 3.5)).toBe(true);
    expect(isTelemetryAvailable(telemetry, 3.501)).toBe(false);
    expect(isTelemetryAvailable(telemetry, 0.9)).toBe(false);
    expect(resourcePressure(telemetry, 1)).toEqual({
      evidence: [],
      pressure: 0,
    });
  });

  it('cites measured thresholds without blaming the target', () => {
    const result = resourcePressure(
      {
        ...telemetry,
        process_cpu_percent: 95,
        system_cpu_percent: 99,
        system_available_memory_bytes: '1024',
        system_total_memory_bytes: '10240',
      },
      1,
    );
    expect(result.pressure).toBe(1);
    expect(result.evidence).toEqual([
      'Native process CPU 95.0% (≥90%)',
      'Host CPU 99.0% (≥90%)',
      'Host available RAM 10.0% (≤10%)',
    ]);
    expect(
      resourcePressure(
        {
          ...telemetry,
          process_cpu_percent: null,
          system_cpu_percent: 95,
        },
        1,
      ).pressure,
    ).toBe(1);
  });

  it('deduplicates native samples and exposes gaps without connecting missing data', () => {
    const points = [
      point(1),
      point(1.25),
      point(1.5),
      point(5, {
        ...telemetry,
        sampled_at_us: '4900000',
      }),
    ];
    const samples = generatorResourcePoints(points);
    expect(samples.map(({ seconds }) => seconds)).toEqual([1, 2.95, 4.9]);
    expect(samples[1]?.telemetry).toBeNull();
    expect(
      generatorResourcePoints([
        point(1),
        point(2, null),
        point(3, {
          ...telemetry,
          sampled_at_us: '2500000',
        }),
      ]).map(({ telemetry: sample }) => sample === null),
    ).toEqual([false, true, false]);
  });

  it('never plots a native timestamp backwards across an unavailable observation', () => {
    expect(
      generatorResourcePoints([
        point(1, null),
        point(1.1, {
          ...telemetry,
          sampled_at_us: '900000',
        }),
        point(2.1, {
          ...telemetry,
          sampled_at_us: '2000000',
        }),
      ]).map(({ seconds }) => seconds),
    ).toEqual([1, 2]);
  });

  it('keeps admission evidence independent of native availability and leaves gaps unfilled', () => {
    const before = point(1, null);
    const after = {
      ...point(2, null),
      capacityDrops: '3',
      missedDeadlines: '2',
    };
    const observations = generatorObservations([before, after]);
    expect(observations[0]?.pressure).toBeNull();
    expect(observations[1]).toEqual({
      seconds: 2,
      pressure: 1,
      evidence: [
        'Resource measurements unavailable',
        '3 capacity drops since preceding retained sample',
        '2 missed arrival slots since preceding retained sample',
      ],
    });
    expect(
      generatorObservations([
        before,
        {
          ...point(1.5, null),
          capacityDrops: null,
          missedDeadlines: null,
        },
        after,
      ])[2]?.pressure,
    ).toBeNull();
  });
});
