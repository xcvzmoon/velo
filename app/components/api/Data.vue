<script setup lang="ts">
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
  async function read(event: Event): Promise<void> {
    const file = event.target instanceof HTMLInputElement ? event.target.files?.[0] : null;
    if (!file) {
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
  <details class="space-y-3 rounded-lg border p-5">
    <summary class="cursor-pointer text-sm font-medium">Shared test data</summary>
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
        <Label for="api-dataset-file">CSV or JSON file (up to 8 MiB)</Label>
        <Input
          :disabled="disabled || busy"
          :accept="definition.source.kind === 'csv' ? '.csv,text/csv' : '.json,application/json'"
          id="api-dataset-file"
          type="file"
          @change="read"
        />
      </div>

      <div class="flex items-center gap-3">
        <Button
          :disabled="disabled || busy"
          variant="outline"
          @click="prepare"
        >
          Prepare records
        </Button>
        <Button
          v-if="busy"
          variant="outline"
          @click="preparation?.abort()"
        >
          Cancel preparation
        </Button>
        <p class="text-muted-foreground text-xs">{{ count }} records prepared in memory.</p>
      </div>
    </template>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>
  </details>
</template>
