import type { AiEvidence, AiMeasurements, AiRun } from '../types/ai';
import type { ComparisonReport } from '../types/contracts';

function measurements(entry: AiRun): AiMeasurements {
  const { snapshot, incomplete, interrupted, stride } = entry;
  return {
    metrics: snapshot.metrics,
    evaluated: snapshot.evaluated,
    measurement: snapshot.measurement,
    arrival: snapshot.arrival,
    telemetry: snapshot.telemetry,
    incomplete,
    interrupted,
    stride,
  };
}

/**
 * Selects measurement populations for native redaction before an explicit cloud submission.
 * @param candidate - Selected run, including its history completeness flags.
 * @param comparison - Selected comparison, or null to explain only the candidate.
 * @returns Measurements without request configuration, release metadata, or history uploads.
 * @example
 * ```ts
 * const evidence = selectAiEvidence(entry, null)
 * ```
 */
export function selectAiEvidence(
  candidate: AiRun,
  comparison: ComparisonReport | null,
): AiEvidence {
  return {
    candidate: measurements(candidate),
    baseline: comparison?.baseline ? measurements(comparison.baseline) : null,
    deltas: comparison?.deltas ?? [],
    comparison_limited: Boolean(comparison?.limitations.length),
    configuration_reviewed: comparison?.configuration_reviewed ?? false,
    configuration_differences: comparison?.differences.length ?? 0,
  };
}
