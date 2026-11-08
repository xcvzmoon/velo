import type { Workload } from '../types/contracts';

type WorkloadPreset = 'smoke' | 'stress' | 'spike' | 'soak';
type PresetSettings = { workload: Workload; warmup_ms: number; pacing_ms: number };

/**
 * Builds editable workload settings within the caller's existing worker budget.
 * @param preset - The test intent, without implying measured machine capacity.
 * @param maxVus - Existing worker budget; validated before applying the preset.
 * @returns Workload settings only. Resource budgets and stop rules stay unchanged.
 * @example
 * ```ts
 * workloadPreset('smoke', 10)
 * ```
 */
export function workloadPreset(preset: WorkloadPreset, maxVus: number): PresetSettings {
  const peak = Math.min(maxVus, 20);
  switch (preset) {
    case 'smoke':
      return {
        workload: {
          kind: 'constant',
          vus: 1,
          duration_ms: 10_000,
        },
        warmup_ms: 0,
        pacing_ms: 0,
      };
    case 'stress':
      return {
        workload: {
          kind: 'ramping',
          start_vus: 1,
          stages: [
            {
              duration_ms: 10_000,
              target_vus: Math.min(maxVus, 5),
            },
            {
              duration_ms: 20_000,
              target_vus: peak,
            },
            {
              duration_ms: 10_000,
              target_vus: peak,
            },
          ],
        },
        warmup_ms: 10_000,
        pacing_ms: 0,
      };
    case 'spike':
      return {
        workload: {
          kind: 'ramping',
          start_vus: 1,
          stages: [
            {
              duration_ms: 5_000,
              target_vus: 1,
            },
            {
              duration_ms: 100,
              target_vus: peak,
            },
            {
              duration_ms: 5_000,
              target_vus: peak,
            },
            {
              duration_ms: 100,
              target_vus: 1,
            },
            {
              duration_ms: 5_000,
              target_vus: 1,
            },
          ],
        },
        warmup_ms: 5_000,
        pacing_ms: 0,
      };
    case 'soak':
      return {
        workload: {
          kind: 'constant',
          vus: Math.min(maxVus, 5),
          duration_ms: 3_600_000,
        },
        warmup_ms: 30_000,
        pacing_ms: 0,
      };
    default:
      return preset satisfies never;
  }
}
