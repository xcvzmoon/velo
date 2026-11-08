import type { ComparisonReport, HistoryEntry, RunSnapshot, TestDefinition } from './contracts';

export type AiTask = 'draft' | 'explain';
export type AiProvider = 'openai' | 'anthropic' | 'gemini' | 'ollama' | 'openai_compatible';
export type AiProviderConfig = {
  provider: AiProvider;
  model: string;
  endpoint: string;
  use_key: boolean;
};
export type AiMeasurements = Pick<
  RunSnapshot,
  'metrics' | 'evaluated' | 'measurement' | 'arrival' | 'telemetry'
> & {
  incomplete: boolean;
  interrupted: boolean;
  stride: number;
};
export type AiEvidence = {
  candidate: AiMeasurements;
  baseline: AiMeasurements | null;
  deltas: ComparisonReport['deltas'];
  comparison_limited: boolean;
  configuration_reviewed: boolean;
  configuration_differences: number;
};
export type AiReview = {
  id: string;
  content: string;
  redactions: string[];
  config: AiProviderConfig;
};
export type AiResult = {
  definition: TestDefinition | null;
  observations: { text: string; evidence: string[] }[];
  hypotheses: string[];
  next_steps: string[];
  assumptions: string[];
  missing_inputs: string[];
  unsupported: string[];
  provider: string;
  model: string;
  endpoint: string;
  generated_at: string;
};
export type AiRun = Pick<HistoryEntry, 'snapshot' | 'incomplete' | 'interrupted' | 'stride'>;
