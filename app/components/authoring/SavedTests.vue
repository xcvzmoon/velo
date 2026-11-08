<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { TestDefinition } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const nameSchema = v.object({
    name: v.pipe(v.string(), v.trim(), v.minLength(1), v.maxLength(80)),
  });
  const props = defineProps<{ source: string; disabled: boolean; canSave: boolean }>();
  const emit = defineEmits<{ load: [definition: TestDefinition] }>();
  const form = useForm({
    schema: nameSchema,
    initialInput: { name: '' },
  });
  const names = shallowRef<string[]>([]);
  const chosen = shallowRef<string>('');
  const error = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);

  async function perform(action: () => Promise<void>): Promise<void> {
    busy.value = true;
    error.value = null;
    try {
      await action();
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    } finally {
      busy.value = false;
    }
  }

  async function refresh(): Promise<void> {
    names.value = await invoke<string[]>('list_definitions');
  }

  async function save(values: InferOutput<typeof nameSchema>): Promise<void> {
    await perform(async () => {
      await invoke('save_definition', {
        name: values.name,
        source: props.source,
      });
      await refresh();
    });
  }

  async function load(): Promise<void> {
    await perform(async () =>
      emit('load', await invoke<TestDefinition>('load_definition', { name: chosen.value })),
    );
  }

  async function remove(): Promise<void> {
    await perform(async () => {
      await invoke('delete_definition', { name: chosen.value });
      chosen.value = '';
      await refresh();
    });
  }

  onMounted(() => perform(refresh));
</script>

<template>
  <ShellPanel
    variant="section"
    aria-label="Saved test definitions"
  >
    <template #header><h2 class="text-highlighted font-semibold">Saved tests</h2></template>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p class="text-muted text-sm">
      Definitions store literal request fields and non-secret variables locally. Use secret
      references for private values. Saving under an existing name replaces that definition.
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
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="test-name"
            >
              Test name
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :disabled="disabled || busy"
              id="test-name"
            />
          </div>

          <p
            v-if="field.errors"
            class="text-error text-sm"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <UButton
        :disabled="disabled || busy || !canSave"
        type="submit"
        label="Save test"
      />
    </FormischForm>

    <template v-if="names.length">
      <div class="grid min-w-0 gap-2">
        <label
          class="text-sm font-medium"
          for="saved-test"
        >
          Saved test
        </label>

        <USelect
          v-model="chosen"
          :disabled="disabled || busy"
          :items="[...names.map((name) => ({ label: name, value: name }))]"
          id="saved-test"
          placeholder="Choose a test"
          class="w-full"
        />
      </div>

      <div class="flex gap-3">
        <UButton
          :disabled="disabled || busy || !chosen"
          variant="outline"
          color="neutral"
          label="Load test"
          @click="load"
        />

        <ShellConfirmAction
          :disabled="disabled || busy || !chosen"
          label="Delete test"
          title="Delete saved test?"
          description="This removes the saved definition. Your current editor and stored results are kept."
          @confirm="remove"
        />
      </div>
    </template>
  </ShellPanel>
</template>
