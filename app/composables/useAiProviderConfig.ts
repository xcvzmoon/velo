import type { Ref } from 'vue';
import type { AiProviderConfig } from '~/types/ai';
import { defaultAiProviderConfig } from '~/schemas/ai';

/**
 * Shares the selected native AI destination across mounted assistant and settings panels.
 * @returns Session-only provider settings, without credentials or persisted chat content. Replace the configuration after schema validation; updates invalidate reviewed submissions.
 * @example
 * ```ts
 * const config = useAiProviderConfig()
 * config.value = defaultAiProviderConfig('openai')
 * ```
 */
export function useAiProviderConfig(): Ref<AiProviderConfig> {
  return useState<AiProviderConfig>('ai-provider-config', () => defaultAiProviderConfig('openai'));
}
