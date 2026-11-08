import type { AiProvider, AiProviderConfig } from '../types/ai';
import * as v from 'valibot';

export const aiProviderLabels: Record<AiProvider, string> = {
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  gemini: 'Gemini',
  ollama: 'Ollama',
  openai_compatible: 'OpenAI-compatible',
};

const modelSchema = v.pipe(
  v.string(),
  v.trim(),
  v.minLength(1, 'Enter a model ID available from this provider.'),
  v.maxLength(200, 'Model IDs must not exceed 200 characters.'),
  v.regex(
    /^[A-Za-z0-9._:/-]+$/,
    'Use letters, digits, dots, hyphens, underscores, colons, or slash-separated names.',
  ),
  v.check(validModelPath, 'Use non-empty model name segments without dot or parent segments.'),
);
const endpointSchema = v.pipe(
  v.string(),
  v.trim(),
  v.maxLength(2048, 'The endpoint must not exceed 2,048 characters.'),
  v.url('Enter an absolute endpoint URL.'),
  v.check(
    validEndpoint,
    'Use HTTPS, or HTTP on localhost or a numeric loopback address. Remove credentials, query parameters, and fragments.',
  ),
);

export const aiProviderSchema = v.picklist([
  'openai',
  'anthropic',
  'gemini',
  'ollama',
  'openai_compatible',
]);

export const aiProviderConfigSchema = v.pipe(
  v.strictObject({
    provider: aiProviderSchema,
    model: modelSchema,
    endpoint: endpointSchema,
    use_key: v.boolean(),
  }),
  v.check(
    validProviderPolicy,
    'Use the fixed native endpoint and a stored key for OpenAI, Anthropic, and Gemini.',
  ),
);

function validModelPath(model: string): boolean {
  return model.split('/').every((part) => part !== '' && part !== '.' && part !== '..');
}

function validProviderPolicy(config: AiProviderConfig): boolean {
  if (config.provider === 'ollama' || config.provider === 'openai_compatible') {
    return true;
  }
  return config.use_key && config.endpoint === defaultAiProviderConfig(config.provider).endpoint;
}

function validEndpoint(endpoint: string): boolean {
  try {
    const url = new URL(endpoint);
    if (
      url.username ||
      url.password ||
      url.port === '0' ||
      endpoint.includes('?') ||
      endpoint.includes('#')
    ) {
      return false;
    }
    if (url.protocol === 'https:') {
      return true;
    }
    return (
      url.protocol === 'http:' &&
      (url.hostname === 'localhost' ||
        url.hostname === '[::1]' ||
        /^127\.\d+\.\d+\.\d+$/.test(url.hostname))
    );
  } catch {
    return false;
  }
}

/**
 * Supplies editable defaults when the user selects a native provider.
 * @param provider - Provider whose endpoint and authentication policy should be selected.
 * @returns A new configuration. Ollama and compatible services require a model ID; compatible services also require an endpoint.
 * @example
 * ```ts
 * const config = defaultAiProviderConfig('ollama')
 * config.model = 'qwen3:8b'
 * ```
 */
export function defaultAiProviderConfig(provider: AiProvider): AiProviderConfig {
  switch (provider) {
    case 'openai':
      return {
        provider,
        model: 'gpt-4.1-mini',
        endpoint: 'https://api.openai.com/v1/chat/completions',
        use_key: true,
      };
    case 'anthropic':
      return {
        provider,
        model: 'claude-sonnet-5-5',
        endpoint: 'https://api.anthropic.com/v1/messages',
        use_key: true,
      };
    case 'gemini':
      return {
        provider,
        model: 'gemini-3.8-flash',
        endpoint: 'https://generativelanguage.googleapis.com/v1beta',
        use_key: true,
      };
    case 'ollama':
      return {
        provider,
        model: '',
        endpoint: 'http://localhost:11434/api/chat',
        use_key: false,
      };
    case 'openai_compatible':
      return {
        provider,
        model: '',
        endpoint: '',
        use_key: true,
      };
    default:
      return provider satisfies never;
  }
}
