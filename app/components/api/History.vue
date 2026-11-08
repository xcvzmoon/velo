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
    if (
      !window.confirm(id ? 'Delete this functional run?' : 'Delete all functional API history?')
    ) {
      return;
    }
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
    <p class="text-muted-foreground text-xs">
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
          <Label for="api-history-limit">Maximum retained runs (0 to 100)</Label>
          <Input
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
            class="text-destructive text-xs"
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
          <Checkbox
            :model-value="field.input"
            :disabled="disabled"
            id="api-history-content"
            @update:model-value="field.input = $event === true"
          />
          <Label for="api-history-content">Persist redacted response content</Label>
        </div>
      </Field>

      <Button
        :disabled="disabled"
        type="submit"
        variant="outline"
      >
        Save retention
      </Button>
      <Button
        :disabled="disabled"
        type="button"
        variant="outline"
        @click="remove(null)"
      >
        Delete all history
      </Button>
    </Form>

    <p
      v-if="error"
      class="text-destructive text-sm"
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
          <Button
            :disabled="disabled"
            size="sm"
            variant="outline"
            @click="emit('inspect', run)"
          >
            Inspect evidence
          </Button>
          <Button
            v-for="result in run.results"
            :key="`${result.iteration}-${result.request_id}`"
            :disabled="disabled"
            size="sm"
            variant="ghost"
            @click="restore(run, result.request_id)"
          >
            Restore {{ result.name }}
          </Button>
          <Button
            :disabled="disabled"
            size="sm"
            variant="ghost"
            @click="remove(run.id)"
          >
            Delete run
          </Button>
        </div>
      </li>
    </ol>
  </details>
</template>
