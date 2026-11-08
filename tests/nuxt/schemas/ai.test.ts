import * as v from 'valibot';
import { describe, expect, it } from 'vitest';
import { aiProviderConfigSchema, defaultAiProviderConfig } from '../../../app/schemas/ai';

describe('native AI provider configuration', () => {
  it('pins cloud providers to their native endpoint and requires stored authentication', () => {
    for (const provider of ['openai', 'anthropic', 'gemini'] as const) {
      const config = defaultAiProviderConfig(provider);
      expect(v.safeParse(aiProviderConfigSchema, config).success).toBe(true);
      expect(
        v.safeParse(aiProviderConfigSchema, {
          ...config,
          use_key: false,
        }).success,
      ).toBe(false);
      expect(
        v.safeParse(aiProviderConfigSchema, {
          ...config,
          endpoint: 'https://other.example/api',
        }).success,
      ).toBe(false);
    }
  });

  it('allows explicit keyless local and secure compatible endpoints with provider model names', () => {
    for (const endpoint of [
      'http://localhost:11434/api/chat',
      'http://127.0.0.1:8080/v1/chat/completions',
      'http://[::1]:11434/api/chat',
      'https://models.example/team/v1/chat/completions',
    ]) {
      const config = {
        provider: 'openai_compatible',
        model: 'organization/model:tag',
        endpoint,
        use_key: false,
      };
      expect(v.safeParse(aiProviderConfigSchema, config).success).toBe(true);
    }
  });

  it('rejects credential-bearing URLs, insecure remote HTTP, and unsupported transports', () => {
    for (const endpoint of [
      'http://models.example/v1/chat/completions',
      'http://127.0.0.1.evil.example/api/chat',
      'https://user:password@models.example/api/chat',
      'https://models.example/api/chat?key=private',
      'https://models.example/api/chat?',
      'https://models.example/api/chat#fragment',
      'https://models.example/api/chat#',
      'file:///private/key',
      'https://models.example:0/api/chat',
    ]) {
      expect(
        v.safeParse(aiProviderConfigSchema, {
          ...defaultAiProviderConfig('ollama'),
          model: 'qwen3:8b',
          endpoint,
        }).success,
      ).toBe(false);
    }
  });

  it('requires complete model and endpoint configuration and rejects control characters', () => {
    expect(v.safeParse(aiProviderConfigSchema, defaultAiProviderConfig('ollama')).success).toBe(
      false,
    );
    expect(
      v.safeParse(aiProviderConfigSchema, defaultAiProviderConfig('openai_compatible')).success,
    ).toBe(false);
    for (const model of [
      '',
      'model\nprivate',
      'model name',
      'a'.repeat(201),
      '../model',
      '/model',
      'model/',
      'org//model',
      'org/./model',
      'model@private',
    ]) {
      expect(
        v.safeParse(aiProviderConfigSchema, {
          ...defaultAiProviderConfig('openai'),
          model,
        }).success,
      ).toBe(false);
    }
    expect(
      v.safeParse(aiProviderConfigSchema, {
        ...defaultAiProviderConfig('openai'),
        key: 'never-accept-secrets-in-config',
      }).success,
    ).toBe(false);
  });
});
