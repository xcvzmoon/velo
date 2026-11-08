<script setup lang="ts">
  import type { FileUploadItem } from '@nuxt/ui';
  import type { DataDefinition } from '~/types/contracts';

  const props = defineProps<{
    definition: DataDefinition | null;
    disabled: boolean;
  }>();
  const emit = defineEmits<{
    apply: [value: DataDefinition | null];
    rows: [value: Record<string, string>[]];
    dirty: [value: boolean];
  }>();
  const source = shallowRef<string>('');
  const count = shallowRef<number>(0);
  const busy = shallowRef<boolean>(false);
  const error = shallowRef<string | null>(null);
  let preparation: AbortController | null = null;
  async function read(file: FileUploadItem | null | undefined): Promise<void> {
    if (!(file instanceof File)) {
      return;
    }
    error.value = null;
    try {
      if (file.size > 8388608) {
        throw new Error('DATASET_BUDGET_EXCEEDED: Choose a file up to 8 MiB.');
      }
      source.value = await file.text();
      count.value = 0;
      emit('rows', []);
    } catch (error_) {
      error.value = error_ instanceof Error ? error_.message : 'DATASET_IMPORT_FAILED';
    }
  }
  async function prepare(): Promise<void> {
    if (!props.definition) {
      return;
    }
    busy.value = true;
    error.value = null;
    preparation = new AbortController();
    try {
      const rows = await prepareData(props.definition, source.value, preparation.signal);
      count.value = rows.length;
      emit('rows', rows);
    } catch (error_) {
      error.value = error_ instanceof Error ? error_.message : 'DATASET_PREPARATION_FAILED';
    } finally {
      busy.value = false;
      preparation = null;
    }
  }
  function apply(value: DataDefinition | null): void {
    source.value = '';
    count.value = 0;
    emit('rows', []);
    emit('apply', value);
  }
  onBeforeUnmount(() => preparation?.abort());
</script>

<template>
  <details class="border-default border-y py-4">
    <summary class="cursor-pointer text-sm font-medium">Shared test data</summary>

    <div class="min-w-0 space-y-4 pt-4">
      <AuthoringDataset
        :definition="definition"
        :disabled="disabled || busy"
        @apply="apply"
        @dirty="emit('dirty', $event)"
      />

      <template v-if="definition">
        <div
          v-if="definition.source.kind !== 'faker'"
          class="space-y-2"
        >
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="api-dataset-file"
            >
              CSV or JSON file (up to 8 MiB)
            </label>

            <UFileUpload
              :disabled="disabled || busy"
              :accept="
                definition.source.kind === 'csv' ? '.csv,text/csv' : '.json,application/json'
              "
              :model-value="null"
              :preview="false"
              id="api-dataset-file"
              label="Choose a CSV or JSON file"
              reset
              @update:model-value="read"
            />
          </div>
        </div>

        <div class="flex items-center gap-3">
          <UButton
            :disabled="disabled || busy"
            variant="outline"
            color="neutral"
            label="Prepare records"
            @click="prepare"
          />

          <UButton
            v-if="busy"
            variant="outline"
            color="neutral"
            label="Cancel preparation"
            @click="preparation?.abort()"
          />

          <p class="text-muted text-xs">{{ count }} records prepared in memory.</p>
        </div>
      </template>

      <p
        v-if="error"
        class="text-error text-sm"
        role="alert"
      >
        {{ error }}
      </p>
    </div>
  </details>
</template>
