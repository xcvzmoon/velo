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
          <Label :for="`${inputId}-provider`">Provider</Label>

          <Select
            :model-value="field.input"
            :disabled="disabled"
            @update:model-value="selectProvider"
          >
            <SelectTrigger :id="`${inputId}-provider`"><SelectValue /></SelectTrigger>

            <SelectContent>
              <SelectItem
                v-for="provider in providers"
                :key="provider"
                :value="provider"
              >
                {{ aiProviderLabels[provider] }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['model']"
      >
        <div class="max-w-lg space-y-2">
          <Label :for="`${inputId}-model`">Model ID</Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :id="`${inputId}-model`"
            :disabled="disabled"
            placeholder="Enter a model available from this provider"
            autocomplete="off"
          />

          <p
            v-if="field.errors"
            class="text-destructive text-sm"
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
          <Label :for="`${inputId}-endpoint`">
            {{ customEndpoint ? 'Complete API endpoint' : 'Provider endpoint' }}
          </Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :id="`${inputId}-endpoint`"
            :disabled="disabled || !customEndpoint"
            placeholder="Enter the complete API endpoint URL"
            type="url"
            autocomplete="off"
            spellcheck="false"
          />

          <p class="text-muted-foreground text-xs">
            {{
              customEndpoint
                ? 'Use the complete chat endpoint. HTTPS is required except for HTTP on localhost or numeric loopback addresses. Keys are scoped to this endpoint.'
                : 'This provider uses its fixed native API. Gemini appends the selected model to this base URL.'
            }}
          </p>

          <p
            v-if="field.errors"
            class="text-destructive text-sm"
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
          <Checkbox
            :id="`${inputId}-key`"
            :model-value="field.input"
            :disabled="disabled"
            @update:model-value="field.input = $event === true"
          />

          <Label :for="`${inputId}-key`">Use the stored API key for this endpoint</Label>
        </div>
      </FormischField>

      <p
        v-if="form.errors"
        class="text-destructive text-sm"
        role="alert"
      >
        {{ form.errors.join(' ') }}
      </p>

      <Button
        :disabled="disabled"
        type="submit"
        variant="outline"
      >
        Use provider configuration
      </Button>
    </FormischForm>

    <p class="text-muted-foreground text-xs">
      Selected: {{ aiProviderLabels[selected.provider] }} · {{ selected.model }} ·
      {{ selected.endpoint }} ·
      {{ selected.use_key ? 'Stored key required' : 'No key; credential store is not accessed' }}.
      Settings remain in this session only. No connection is made when selecting a configuration.
    </p>

    <p
      v-if="message"
      class="text-muted-foreground text-sm"
      role="status"
    >
      {{ message }}
    </p>
  </div>
</template>
