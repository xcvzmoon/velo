import type { Mock } from 'vitest';
import type { AiProviderConfig, AiResult, AiReview } from '../../../app/types/ai';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { effectScope, onScopeDispose, shallowRef } from 'vue';
import { useAiAssistant } from '../../../app/composables/useAiAssistant';
import { formatAuthoringError } from '../../../app/utils/authoring';

const scopes: ReturnType<typeof effectScope>[] = [];
const config: AiProviderConfig = {
  provider: 'openai',
  model: 'gpt-4.1-mini',
  endpoint: 'https://api.openai.com/v1/chat/completions',
  use_key: true,
};
const reviewed: AiReview = {
  config,
  id: 'review-one',
  content: '{"safe":true}',
  redactions: [],
};
const response: AiResult = {
  definition: null,
  observations: [],
  hypotheses: [],
  next_steps: [],
  assumptions: [],
  missing_inputs: [],
  unsupported: [],
  provider: 'openai',
  model: 'gpt-4.1-mini',
  endpoint: config.endpoint,
  generated_at: '1234567890',
};

beforeEach(() => {
  vi.stubGlobal('shallowRef', shallowRef);
  vi.stubGlobal('onScopeDispose', onScopeDispose);
  vi.stubGlobal('formatAuthoringError', formatAuthoringError);
});
afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
  vi.unstubAllGlobals();
});

function createAssistant(
  transport: Parameters<typeof useAiAssistant>[0],
): ReturnType<typeof useAiAssistant> {
  const scope = effectScope();
  scopes.push(scope);
  const assistant = scope.run(() => useAiAssistant(transport));
  if (!assistant) {
    throw new Error('The assistant scope must be active.');
  }
  return assistant;
}

type AiTransport = NonNullable<Parameters<typeof useAiAssistant>[0]>;
type TestTransport = {
  prepare: Mock<AiTransport['prepare']>;
  generate: Mock<AiTransport['generate']>;
  cancel: Mock<AiTransport['cancel']>;
};

function createTransport(): TestTransport {
  return {
    prepare: vi.fn(() => Promise.resolve({ ...reviewed })),
    generate: vi.fn(() => Promise.resolve(response)),
    cancel: vi.fn(() => Promise.resolve()),
  };
}

describe('useAiAssistant lifecycle', () => {
  it('copies provider settings at the review boundary and submits only the native review ID', async () => {
    const preparation = Promise.withResolvers<AiReview>();
    const transport = createTransport();
    transport.prepare.mockReturnValue(preparation.promise);
    const assistant = createAssistant(transport);
    const selected = { ...config };
    const pending = assistant.prepare('draft', 'Draft a request', null, null, selected);
    selected.endpoint = 'https://another.example/v1/chat/completions';
    selected.model = 'changed-model';
    expect(transport.prepare.mock.calls[0]?.[4]).toEqual(config);
    preparation.resolve(reviewed);
    await pending;
    await assistant.generate();
    expect(transport.generate).toHaveBeenCalledExactlyOnceWith('review-one');
  });
  it('uses only the frozen review ID for generation and ignores overlapping generation', async () => {
    const generation = Promise.withResolvers<AiResult>();
    const transport = createTransport();
    transport.generate.mockReturnValue(generation.promise);
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Draft a health check', null, null, config);
    expect(transport.prepare).toHaveBeenCalledWith(
      'draft',
      'Draft a health check',
      null,
      null,
      config,
    );
    expect(transport.generate).not.toHaveBeenCalled();

    const first = assistant.generate();
    assistant.review.value = {
      ...reviewed,
      id: 'unsubmitted-review',
    };
    await assistant.generate();
    expect(transport.generate).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.busy.value).toBe(true);
    generation.resolve(response);
    await first;
    expect(assistant.result.value).toEqual(response);
    expect(assistant.review.value).toBeNull();
    expect(assistant.busy.value).toBe(false);
  });

  it('discards a cancelled generation and preserves a newer prepared review', async () => {
    const generation = Promise.withResolvers<AiResult>();
    const transport = createTransport();
    transport.generate.mockReturnValue(generation.promise);
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Initial request', null, null, config);
    const first = assistant.generate();
    await assistant.cancel();
    transport.prepare.mockResolvedValue({
      ...reviewed,
      id: 'review-two',
    });
    await assistant.prepare('draft', 'Replacement request', null, null, config);
    generation.resolve(response);
    await first;
    expect(transport.cancel).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.result.value).toBeNull();
    expect(assistant.review.value?.id).toBe('review-two');
    expect(assistant.busy.value).toBe(false);
  });

  it('cleans up a late prepared review after reset without replacing newer state', async () => {
    const preparation = Promise.withResolvers<AiReview>();
    const transport = createTransport();
    transport.prepare.mockReturnValueOnce(preparation.promise);
    const assistant = createAssistant(transport);
    const first = assistant.prepare('draft', 'Old request', null, null, config);
    assistant.reset();
    transport.prepare.mockResolvedValue({
      ...reviewed,
      id: 'review-two',
    });
    await assistant.prepare('draft', 'New request', null, null, config);
    preparation.resolve(reviewed);
    await first;
    expect(transport.cancel).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.review.value?.id).toBe('review-two');
    expect(assistant.error.value).toBeNull();
  });

  it('cancels replaced and reset prepared reviews using only their owned IDs', async () => {
    const transport = createTransport();
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'First request', null, null, config);
    transport.prepare.mockResolvedValue({
      ...reviewed,
      id: 'review-two',
    });
    await assistant.prepare('draft', 'Second request', null, null, config);
    expect(transport.cancel).toHaveBeenCalledWith('review-one');
    assistant.reset();
    expect(transport.cancel).toHaveBeenCalledWith('review-two');
    expect(assistant.review.value).toBeNull();
    expect(assistant.result.value).toBeNull();
  });

  it('does not let a late cancellation failure overwrite a newer operation', async () => {
    const cancellation = Promise.withResolvers<undefined>();
    const transport = createTransport();
    transport.cancel.mockReturnValueOnce(cancellation.promise);
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Initial request', null, null, config);
    const pending = assistant.cancel();
    await assistant.prepare('draft', 'Next request', null, null, config);
    cancellation.reject(new Error('PRIVATE_INFRASTRUCTURE_DATA'));
    await pending;
    expect(assistant.error.value).toBeNull();
    expect(assistant.review.value).toEqual(reviewed);
  });

  it('shows safe failures, clears pending state, and allows retry', async () => {
    const transport = createTransport();
    transport.prepare.mockRejectedValueOnce(new Error('PRIVATE_INFRASTRUCTURE_DATA'));
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Initial request', null, null, config);
    expect(assistant.error.value).toBe('AUTHORING_FAILED: The operation could not complete.');
    expect(assistant.busy.value).toBe(false);
    await assistant.prepare('draft', 'Retry request', null, null, config);
    transport.generate.mockRejectedValueOnce({
      code: 'AI_PROVIDER_UNAVAILABLE',
      message: 'Try again later.',
      field: null,
    });
    await assistant.generate();
    expect(assistant.error.value).toBe('AI_PROVIDER_UNAVAILABLE: Try again later.');
    expect(assistant.busy.value).toBe(false);
    expect(assistant.review.value).toBeNull();
    expect(assistant.result.value).toBeNull();
  });

  it('ignores overlapping preparations and cleans up after scope disposal', async () => {
    const preparation = Promise.withResolvers<AiReview>();
    const transport = createTransport();
    transport.prepare.mockReturnValue(preparation.promise);
    const assistant = createAssistant(transport);
    const pending = assistant.prepare('draft', 'First request', null, null, config);
    await assistant.prepare('draft', 'Ignored request', null, null, config);
    expect(transport.prepare).toHaveBeenCalledTimes(1);
    for (const scope of scopes) {
      scope.stop();
    }
    preparation.resolve(reviewed);
    await pending;
    expect(transport.cancel).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.review.value).toBeNull();
    expect(assistant.busy.value).toBe(false);
    await assistant.prepare('draft', 'Disposed request', null, null, config);
    expect(transport.prepare).toHaveBeenCalledTimes(1);
  });

  it('cancels an active generation on scope disposal and rejects its late response', async () => {
    const generation = Promise.withResolvers<AiResult>();
    const transport = createTransport();
    transport.generate.mockReturnValue(generation.promise);
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Initial request', null, null, config);
    const pending = assistant.generate();
    for (const scope of scopes) {
      scope.stop();
    }
    generation.resolve(response);
    await pending;
    expect(transport.cancel).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.result.value).toBeNull();
    expect(assistant.error.value).toBeNull();
  });

  it('resets an active generation and ignores both late rejection and cleanup failure', async () => {
    const generation = Promise.withResolvers<AiResult>();
    const cancellation = Promise.withResolvers<undefined>();
    const transport = createTransport();
    transport.generate.mockReturnValue(generation.promise);
    transport.cancel.mockReturnValueOnce(cancellation.promise);
    const assistant = createAssistant(transport);
    await assistant.prepare('draft', 'Initial request', null, null, config);
    const pending = assistant.generate();
    assistant.reset();
    generation.reject(new Error('PRIVATE_PROVIDER_ERROR'));
    cancellation.reject(new Error('PRIVATE_CANCEL_ERROR'));
    await pending;
    expect(transport.cancel).toHaveBeenCalledExactlyOnceWith('review-one');
    expect(assistant.review.value).toBeNull();
    expect(assistant.result.value).toBeNull();
    expect(assistant.error.value).toBeNull();
    expect(assistant.busy.value).toBe(false);
  });
});
