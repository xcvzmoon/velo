import { describe, expect, it } from 'vitest';
import { hasIsolatedChartPoints } from '../../../app/utils/chart';

describe('isolated chart observations', () => {
  it('keeps retained observations visible between compaction gaps', () => {
    expect(hasIsolatedChartPoints([20, null, 40, null, 60], 0, 4)).toBe(true);
    expect(hasIsolatedChartPoints([null, 40, null], 0, 2)).toBe(true);
    expect(hasIsolatedChartPoints([undefined, 40, undefined], 0, 2)).toBe(true);
  });

  it('does not require dots for continuous lines or invent missing observations', () => {
    expect(hasIsolatedChartPoints([20, 40, 60], 0, 2)).toBe(false);
    expect(hasIsolatedChartPoints([20, 40, null, 60, 80], 0, 4)).toBe(false);
    expect(hasIsolatedChartPoints([null, null], 0, 1)).toBe(false);
    expect(hasIsolatedChartPoints([], 0, 0)).toBe(false);
  });

  it('handles a single observation and clipped visible ranges', () => {
    expect(hasIsolatedChartPoints([0], 0, 0)).toBe(true);
    expect(hasIsolatedChartPoints([20, 40, 60], 1, 1)).toBe(true);
    expect(hasIsolatedChartPoints([20, null, 60], 0, 1)).toBe(true);
  });
});
