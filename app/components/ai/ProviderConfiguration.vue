<script setup lang="ts">
  import type { AiProvider, AiProviderConfig } from '~/types/ai';
  import {
    Field as FormischField,
    Form as FormischForm,
    getInput,
    setInput,
    useForm,
  } from '@formisch/vue';
  import * as v from 'valibot';
  import {
    aiProviderConfigSchema,
    aiProviderLabels,
    aiProviderSchema,
    defaultAiProviderConfig,
  } from '~/schemas/ai';

  const providers: AiProvider[] = ['openai', 'anthropic', 'gemini', 'ollama', 'openai_compatible'];
  const providerItems = providers.map((provider) => ({
    label: aiProviderLabels[provider],
    value: provider,
  }));

  const props = defineProps<{ disabled: boolean }>();
  const emit = defineEmits<{ dirty: [value: boolean] }>();
  const selected = useAiProviderConfig();
  const inputId = useId();
  const form = useForm({
    schema: aiProviderConfigSchema,
    initialInput: { ...selected.value },
  });
  const message = shallowRef<string | null>(null);
  const dirty = computed<boolean>(() => {
    const input = getInput(form);
    const current = selected.value;
    return (
      input.provider !== current.provider ||
      input.model !== current.model ||
      input.endpoint !== current.endpoint ||
      input.use_key !== current.use_key
    );
  });
  const customEndpoint = computed<boolean>(() => {
    const { provider } = getInput(form);
    return provider === 'ollama' || provider === 'openai_compatible';
  });

  function selectProvider(value: Parameters<typeof v.safeParse>[1]): void {
    const parsed = v.safeParse(aiProviderSchema, value);
    if (parsed.success) {
      setInput(form, { input: defaultAiProviderConfig(parsed.output) });
      message.value = null;
    }
  }

  function apply(config: AiProviderConfig): void {
    if (props.disabled) {
      return;
    }
    selected.value = { ...config };
    message.value =
      'Provider configuration selected for this session. Review the destination before submitting data.';
  }

  watch(dirty, (value) => emit('dirty', value), { immediate: true });
  watch(selected, (config) => {
    setInput(form, { input: { ...config } });
  });
</script>

<template>
  <div
    class="space-y-4"
    aria-label="Native AI provider configuration"
  >
    <FormischForm
      :of="form"
      class="space-y-4"
      @submit="apply"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['provider']"
      >
        <div class="max-w-sm space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${inputId}-provider`"
              class="text-sm font-medium"
            >
              Provider
            </label>

            <USelect
              :model-value="field.input"
              :disabled="disabled"
              :id="`${inputId}-provider`"
              :items="providerItems"
              class="w-full"
              @update:model-value="selectProvider"
            />
          </div>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['model']"
      >
        <div class="max-w-lg space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${inputId}-model`"
              class="text-sm font-medium"
            >
              Model ID
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${inputId}-model`"
              :disabled="disabled"
              placeholder="Enter a model available from this provider"
              autocomplete="off"
            />
          </div>

          <p
            v-if="field.errors"
            class="text-error text-sm"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['endpoint']"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${inputId}-endpoint`"
              class="text-sm font-medium"
            >
              {{ customEndpoint ? 'Complete API endpoint' : 'Provider endpoint' }}
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${inputId}-endpoint`"
              :disabled="disabled || !customEndpoint"
              placeholder="Enter the complete API endpoint URL"
              type="url"
              autocomplete="off"
              spellcheck="false"
            />
          </div>

          <p class="text-muted text-xs">
            {{
              customEndpoint
                ? 'Use the complete chat endpoint. HTTPS is required except for HTTP on localhost or numeric loopback addresses. Keys are scoped to this endpoint.'
                : 'This provider uses its fixed native API. Gemini appends the selected model to this base URL.'
            }}
          </p>

          <p
            v-if="field.errors"
            class="text-error text-sm"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-if="customEndpoint"
        v-slot="field"
        :of="form"
        :path="['use_key']"
      >
        <div class="flex items-center gap-2">
          <UCheckbox
            :id="`${inputId}-key`"
            :model-value="field.input"
            :disabled="disabled"
            @update:model-value="field.input = $event === true"
          />

          <label
            :for="`${inputId}-key`"
            class="text-sm font-medium"
          >
            Use the stored API key for this endpoint
          </label>
        </div>
      </FormischField>

      <p
        v-if="form.errors"
        class="text-error text-sm"
        role="alert"
      >
        {{ form.errors.join(' ') }}
      </p>

      <UButton
        :disabled="disabled"
        type="submit"
        variant="outline"
        color="neutral"
        label="Use provider configuration"
      />
    </FormischForm>

    <p class="text-muted text-xs">
      Selected: {{ aiProviderLabels[selected.provider] }} · {{ selected.model }} ·
      {{ selected.endpoint }} ·
      {{ selected.use_key ? 'Stored key required' : 'No key; credential store is not accessed' }}.
      Settings remain in this session only. No connection is made when selecting a configuration.
    </p>

    <p
      v-if="message"
      class="text-muted text-sm"
      role="status"
    >
      {{ message }}
    </p>
  </div>
</template>
