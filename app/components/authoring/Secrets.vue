<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import { Field as FormischField, Form as FormischForm, reset, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const schema = v.object({
    name: v.pipe(v.string(), v.regex(/^[A-Za-z0-9_.-]{1,80}$/)),
    value: v.pipe(v.string(), v.maxLength(16_384)),
  });
  defineProps<{ disabled: boolean }>();
  const emit = defineEmits<{ change: [] }>();
  const form = useForm({
    schema,
    initialInput: {
      name: '',
      value: '',
    },
  });
  const message = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);

  async function save(values: InferOutput<typeof schema>): Promise<void> {
    busy.value = true;
    message.value = null;
    try {
      await invoke('set_authoring_secret', {
        name: values.name,
        value: values.value || null,
      });
      message.value = values.value
        ? 'Secret stored in the OS credential store.'
        : 'Secret deleted.';
      reset(form);
      emit('change');
    } catch (error) {
      message.value = formatAuthoringError(error);
    } finally {
      busy.value = false;
    }
  }
</script>

<template>
  <section
    class="space-y-4 rounded-lg border p-6"
    aria-label="Secret references"
  >
    <h2 class="font-medium">OS secrets</h2>

    <p class="text-muted text-sm">
      Use
      <code v-text="'{{secret:name}}'" />
      in request fields. Values stay in the OS credential store and are excluded from definitions
      and exports. An empty value deletes the named secret.
    </p>

    <FormischForm
      :of="form"
      class="flex flex-wrap items-end gap-3"
      @submit="save"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['name']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="secret-name"
          >
            Secret name
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled || busy"
            id="secret-name"
            autocomplete="off"
          />

          <p
            v-if="field.errors"
            class="text-error text-sm"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['value']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="secret-value"
          >
            Value
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled || busy"
            id="secret-value"
            type="password"
            autocomplete="new-password"
          />

          <p
            v-if="field.errors"
            class="text-error text-sm"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <UButton
        :disabled="disabled || busy"
        type="submit"
        label="Update OS secret"
      />
    </FormischForm>

    <p
      v-if="message"
      class="text-sm"
      role="status"
    >
      {{ message }}
    </p>
  </section>
</template>
