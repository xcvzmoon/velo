<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import { Field as FormischField, Form as FormischForm, reset, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';
  import { aiProviderConfigSchema, aiProviderLabels } from '~/schemas/ai';

  const keySchema = v.object({
    value: v.pipe(
      v.string(),
      v.trim(),
      v.minLength(1, 'Enter a provider API key.'),
      v.maxLength(4096, 'The key must not exceed 4,096 characters.'),
      v.regex(/^[\x21-\x7E]+$/, 'The key must contain printable characters without spaces.'),
    ),
  });
  const selected = useAiProviderConfig();
  const form = useForm({
    schema: keySchema,
    initialInput: { value: '' },
  });
  const inputId = useId();
  const busy = shallowRef<boolean>(false);
  const configDirty = shallowRef<boolean>(false);
  const error = shallowRef<string | null>(null);
  const message = shallowRef<string | null>(null);

  let revision = 0;

  async function update(value: string | null): Promise<void> {
    if (busy.value || configDirty.value) {
      return;
    }
    const parsed = v.safeParse(aiProviderConfigSchema, selected.value);
    if (!parsed.success) {
      error.value =
        'AI_INVALID_CONFIGURATION: Select a valid provider configuration before changing its key.';
      return;
    }
    const config = parsed.output;
    const current = revision;
    busy.value = true;
    error.value = null;
    message.value = null;
    try {
      await invoke('set_ai_key', {
        provider: config.provider,
        endpoint: config.endpoint,
        value,
      });
      reset(form);
      if (current === revision) {
        message.value = value
          ? 'Provider key stored in the OS credential store for this endpoint.'
          : 'Provider key removed from the OS credential store for this endpoint.';
      }
    } catch (error_) {
      if (current === revision) {
        error.value = formatAuthoringError(error_);
      }
    } finally {
      busy.value = false;
    }
  }

  async function save(values: InferOutput<typeof keySchema>): Promise<void> {
    await update(values.value);
  }

  async function remove(): Promise<void> {
    await update(null);
  }

  watch(selected, () => {
    revision++;
    reset(form);
    message.value = null;
    error.value = null;
  });
  onBeforeUnmount(() => reset(form));
</script>

<template>
  <section
    class="space-y-5 rounded-lg border p-6"
    aria-label="AI provider settings"
  >
    <h2 class="font-medium">Native AI assistance</h2>

    <p class="text-muted text-sm">
      Choose OpenAI, Anthropic, Gemini, Ollama, or an OpenAI-compatible service. Manual authoring
      and execution remain available without an AI provider. Review the destination and selected
      content before every submission. Selecting a configuration or storing a key sends no test
      data.
    </p>

    <AiProviderConfiguration
      :disabled="busy"
      @dirty="configDirty = $event"
    />

    <div class="space-y-2">
      <h3 class="text-sm font-medium">Key for {{ aiProviderLabels[selected.provider] }}</h3>

      <p class="text-muted text-sm break-all">{{ selected.endpoint }}</p>

      <p
        v-if="!selected.use_key"
        class="text-muted text-sm"
      >
        This configuration sends requests without a key and does not access the credential store.
        Enable stored-key authentication above if your endpoint requires it.
      </p>
    </div>

    <FormischForm
      :of="form"
      class="space-y-4"
      @submit="save"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['value']"
      >
        <div class="max-w-lg space-y-2">
          <label
            :for="inputId"
            class="text-sm font-medium"
          >
            Provider API key
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="inputId"
            :disabled="busy || configDirty || !selected.use_key"
            type="password"
            autocomplete="off"
            spellcheck="false"
          />

          <p
            v-if="field.errors"
            class="text-error text-sm"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <div class="flex flex-wrap gap-3">
        <UButton
          :disabled="busy || configDirty || !selected.use_key"
          type="submit"
          label="Store key securely"
        />

        <UButton
          :disabled="busy || configDirty"
          type="button"
          variant="outline"
          color="neutral"
          label="Remove endpoint key"
          @click="remove"
        />
      </div>
    </FormischForm>

    <p class="text-muted text-xs">
      Keys stay in the OS credential store and are scoped to the selected provider and normalized
      endpoint. Changing an endpoint never reuses another endpoint's key. Keys are not read back
      into this form, model context, test definitions, or reports.
    </p>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p
      v-if="message"
      class="text-sm"
      role="status"
    >
      {{ message }}
    </p>
  </section>
</template>
