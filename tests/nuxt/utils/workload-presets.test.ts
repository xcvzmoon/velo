import { describe, expect, it } from 'vitest';
import { workloadPreset } from '../../../app/utils/workload-presets';

describe('workload presets', () => {
  it.each(['smoke', 'stress', 'spike', 'soak'] as const)(
    'keeps %s within the existing worker budget',
    (preset) => {
      const { workload, warmup_ms } = workloadPreset(preset, 2);
      if (workload.kind === 'constant') {
        expect(workload.vus).toBeLessThanOrEqual(2);
        expect(workload.duration_ms).toBeGreaterThan(warmup_ms);
      } else if (workload.kind === 'ramping') {
        expect(workload.start_vus).toBeLessThanOrEqual(2);
        let duration = 0;
        for (const stage of workload.stages) {
          expect(stage.target_vus).toBeLessThanOrEqual(2);
          duration += stage.duration_ms;
        }
        expect(duration).toBeGreaterThan(warmup_ms);
      }
    },
  );

  it('provides a rapid, reversible spike and a one-hour soak', () => {
    const spike = workloadPreset('spike', 100).workload;
    expect(spike.kind).toBe('ramping');
    if (spike.kind === 'ramping') {
      expect(spike.stages[1]).toEqual({
        duration_ms: 100,
        target_vus: 20,
      });
      expect(spike.stages.at(-1)?.target_vus).toBe(1);
    }
    expect(workloadPreset('soak', 100).workload).toEqual({
      kind: 'constant',
      vus: 5,
      duration_ms: 3_600_000,
    });
  });
});
