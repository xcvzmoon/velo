<script setup lang="ts">
  import type { CollectionRequest, FunctionalRun } from '~/types/contracts';
  import { Field, Form, useForm, setInput } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const props = defineProps<{
    disabled: boolean;
    revision: string | null;
  }>();
  const emit = defineEmits<{
    restore: [snapshot: CollectionRequest];
    inspect: [run: FunctionalRun];
  }>();
  const history = shallowRef<FunctionalRun[]>([]);
  const error = shallowRef<string | null>(null);
  const schema = v.object({
    limit: v.pipe(
      v.union([v.number(), v.string()]),
      v.toNumber(),
      v.integer(),
      v.minValue(0),
      v.maxValue(100),
    ),
    persist_content: v.boolean(),
  });
  const form = useForm({
    schema,
    initialInput: {
      limit: 25,
      persist_content: false,
    },
  });
  async function refresh(): Promise<void> {
    try {
      history.value = await invoke<FunctionalRun[]>('api_history');
      const settings = await invoke<{
        limit: number;
        persist_content: boolean;
      }>('api_history_settings');
      setInput(form, { input: settings });
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    }
  }
  async function save(settings: v.InferOutput<typeof schema>): Promise<void> {
    try {
      await invoke('set_api_history_settings', {
        limit: settings.limit,
        persistContent: settings.persist_content,
      });
      await refresh();
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    }
  }
  async function remove(id: string | null): Promise<void> {
    try {
      await invoke('delete_api_history', { id });
      await refresh();
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    }
  }
  function restore(run: FunctionalRun, id: string): void {
    const snapshot = run.requests.find((request) => request.id === id);
    if (snapshot) {
      emit('restore', snapshot);
    } else {
      error.value = 'REQUEST_UNAVAILABLE: This history entry has no authored request snapshot.';
    }
  }
  watch(() => props.revision, refresh, { immediate: true });
</script>

<template>
  <details class="space-y-4 rounded-lg border p-5">
    <summary class="cursor-pointer font-medium">Functional API history</summary>

    <p class="text-muted text-xs">
      History is separate from load-test results. Response content is excluded by default. Enabling
      content persistence stores only the bounded, redacted response.
    </p>

    <Form
      :of="form"
      class="flex flex-wrap items-end gap-4"
      @submit="save"
    >
      <Field
        v-slot="field"
        :of="form"
        :path="['limit']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="api-history-limit"
          >
            Maximum retained runs (0 to 100)
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled"
            id="api-history-limit"
            type="number"
            min="0"
            max="100"
          />

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </Field>

      <Field
        v-slot="field"
        :of="form"
        :path="['persist_content']"
      >
        <div class="flex items-center gap-2">
          <UCheckbox
            :model-value="field.input"
            :disabled="disabled"
            id="api-history-content"
            @update:model-value="field.input = $event === true"
          />

          <label
            class="text-sm font-medium"
            for="api-history-content"
          >
            Persist redacted response content
          </label>
        </div>
      </Field>

      <UButton
        :disabled="disabled"
        type="submit"
        variant="outline"
        color="neutral"
        label="Save retention"
      />

      <ShellConfirmAction
        :disabled="disabled"
        label="Delete all history"
        title="Delete all API history?"
        description="Stored functional evidence will be permanently removed."
        @confirm="remove(null)"
      />
    </Form>

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <ol class="space-y-3">
      <li
        v-for="run in history"
        :key="run.id"
        class="space-y-2 border-t pt-3"
      >
        <p class="text-sm">
          {{ run.state }} · {{ run.results.length }} recorded outcomes ·
          {{ run.environment ?? 'shared variables' }}
        </p>

        <div class="flex flex-wrap gap-2">
          <UButton
            :disabled="disabled"
            size="sm"
            variant="outline"
            color="neutral"
            label="Inspect evidence"
            @click="emit('inspect', run)"
          />

          <UButton
            v-for="result in run.results"
            :key="`${result.iteration}-${result.request_id}`"
            :disabled="disabled"
            :label="`Restore ${result.name}`"
            size="sm"
            variant="ghost"
            color="neutral"
            @click="restore(run, result.request_id)"
          />

          <ShellConfirmAction
            :disabled="disabled"
            label="Delete run"
            title="Delete API run?"
            description="Stored functional evidence will be permanently removed."
            @confirm="remove(run.id)"
          />
        </div>
      </li>
    </ol>
  </details>
</template>
