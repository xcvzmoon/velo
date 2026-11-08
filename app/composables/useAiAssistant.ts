import type { AiEvidence, AiProviderConfig, AiResult, AiReview, AiTask } from '~/types/ai';
import type { TestDefinition } from '~/types/contracts';
import { invoke } from '@tauri-apps/api/core';

type AiTransport = {
  prepare: (
    task: AiTask,
    description: string,
    definition: TestDefinition | null,
    evidence: AiEvidence | null,
    config: AiProviderConfig,
  ) => Promise<AiReview>;
  generate: (reviewId: string) => Promise<AiResult>;
  cancel: (reviewId: string) => Promise<void>;
};

const nativeTransport: AiTransport = {
  prepare: invokeReview,
  generate: invokeGeneration,
  cancel: invokeCancellation,
};

type AiAssistant = {
  review: ShallowRef<AiReview | null>;
  result: ShallowRef<AiResult | null>;
  error: ShallowRef<string | null>;
  busy: ShallowRef<boolean>;
  prepare: (
    task: AiTask,
    description: string,
    definition: TestDefinition | null,
    evidence: AiEvidence | null,
    config: AiProviderConfig,
  ) => Promise<void>;
  generate: () => Promise<void>;
  reset: () => void;
  cancel: () => Promise<void>;
};

/**
 * Owns one assistant panel's review and response for its reactive scope.
 * @param transport - Native review, generation, and cancellation commands by default; replaceable in tests.
 * @returns Ephemeral state and explicit review, submission, cancellation, and reset actions.
 * Native failures populate error; a cancelled or stale operation cannot publish its response.
 * @example
 * ```ts
 * const assistant = useAiAssistant()
 * const config = { provider: 'openai', model: 'gpt-4.1-mini', endpoint: 'https://api.openai.com/v1/chat/completions', use_key: true } satisfies AiProviderConfig
 * await assistant.prepare('draft', 'Draft a health check', null, null, config)
 * await assistant.generate()
 * ```
 */
export function useAiAssistant(transport: AiTransport = nativeTransport): AiAssistant {
  const review = shallowRef<AiReview | null>(null);
  const result = shallowRef<AiResult | null>(null);
  const error = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  let revision = 0;
  let generating: string | null = null;

  let disposed = false;

  function invalidate(): string | null {
    revision++;
    const reviewId = generating ?? review.value?.id ?? null;
    review.value = null;
    busy.value = false;
    generating = null;
    return reviewId;
  }

  async function discard(reviewId: string, current: number | null): Promise<void> {
    try {
      await transport.cancel(reviewId);
    } catch {
      if (current === revision && !disposed) {
        error.value =
          'AI_CANCEL_FAILED: The request may still be completing. Its response will be discarded.';
      }
    }
  }

  async function cancel(): Promise<void> {
    const reviewId = invalidate();
    if (reviewId) {
      await discard(reviewId, revision);
    }
  }

  function reset(): void {
    const reviewId = invalidate();
    result.value = null;
    error.value = null;
    if (reviewId) {
      void discard(reviewId, null);
    }
  }

  async function prepare(
    task: AiTask,
    description: string,
    definition: TestDefinition | null,
    evidence: AiEvidence | null,
    config: AiProviderConfig,
  ): Promise<void> {
    if (busy.value || disposed) {
      return;
    }
    const previousReviewId = review.value?.id;
    const current = ++revision;
    if (previousReviewId) {
      void discard(previousReviewId, null);
    }
    busy.value = true;
    error.value = null;
    review.value = null;
    result.value = null;
    try {
      const value = await transport.prepare(task, description, definition, evidence, { ...config });
      if (current === revision) {
        review.value = value;
      } else {
        await discard(value.id, null);
      }
    } catch (error_) {
      if (current === revision) {
        error.value = formatAuthoringError(error_);
      }
    } finally {
      if (current === revision) {
        busy.value = false;
      }
    }
  }

  async function generate(): Promise<void> {
    if (busy.value || !review.value || disposed) {
      return;
    }
    const current = ++revision;
    const reviewId = review.value.id;
    busy.value = true;
    generating = reviewId;
    error.value = null;
    try {
      const value = await transport.generate(reviewId);
      if (current === revision) {
        result.value = value;
      }
    } catch (error_) {
      if (current === revision) {
        error.value = formatAuthoringError(error_);
      }
    } finally {
      if (current === revision) {
        generating = null;
        busy.value = false;
        review.value = null;
      }
    }
  }

  onScopeDispose(() => {
    disposed = true;
    reset();
  });
  return { review, result, error, busy, prepare, generate, reset, cancel };
}

function invokeReview(
  task: AiTask,
  description: string,
  definition: TestDefinition | null,
  evidence: AiEvidence | null,
  config: AiProviderConfig,
): Promise<AiReview> {
  return invoke<AiReview>('prepare_ai_context', {
    task,
    description,
    definition,
    evidence,
    config,
  });
}

function invokeGeneration(reviewId: string): Promise<AiResult> {
  return invoke<AiResult>('generate_ai', { reviewId });
}

async function invokeCancellation(reviewId: string): Promise<void> {
  await invoke('cancel_ai', { reviewId });
}
