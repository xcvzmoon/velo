<script setup lang="ts">
  import type { ImportedRequest, ImportFormat, RunConfig } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  const props = defineProps<{ disabled: boolean }>();
  const emit = defineEmits<{ apply: [config: RunConfig] }>();
  const format = shallowRef<ImportFormat>('curl');
  const candidates = shallowRef<ImportedRequest[]>([]);
  const selected = shallowRef<string>('0');
  const error = shallowRef<string | null>(null);
  const busy = shallowRef<boolean>(false);
  const candidate = computed<ImportedRequest | null>(
    () => candidates.value[Number(selected.value)] ?? null,
  );

  async function convert(source: string): Promise<void> {
    busy.value = true;
    error.value = null;
    candidates.value = [];
    try {
      candidates.value = await invoke<ImportedRequest[]>('import_requests', {
        format: format.value,
        source,
      });
      selected.value = '0';
    } catch (error_) {
      error.value = formatAuthoringError(error_);
    } finally {
      busy.value = false;
    }
  }
</script>

<template>
  <section
    class="space-y-4 rounded-lg border p-6"
    aria-label="Request import"
  >
    <h2 class="font-medium">Import a request</h2>

    <p class="text-muted text-sm">
      Paste a cURL command, OpenAPI 3 JSON, or Postman v2 collection JSON. Conversion runs offline.
      Scripts and external references never execute.
    </p>

    <label
      class="text-sm font-medium"
      for="import-format"
    >
      Source format
    </label>

    <USelect
      v-model="format"
      :disabled="disabled || busy"
      :items="[
        { label: 'cURL', value: 'curl' },
        { label: 'OpenAPI 3 JSON', value: 'openapi' },
        { label: 'Postman v2 JSON', value: 'postman' },
      ]"
      id="import-format"
      class="w-full"
    />

    <AuthoringTextForm
      :disabled="disabled || busy"
      source=""
      label="Request source"
      action="Convert requests"
      @apply="convert"
    />

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <template v-if="candidate">
      <label
        class="text-sm font-medium"
        for="import-operation"
      >
        Request to import
      </label>

      <USelect
        v-model="selected"
        :disabled="disabled || busy"
        :items="[
          ...candidates.map((entry, index) => ({ label: entry.name, value: String(index) })),
        ]"
        id="import-operation"
        class="w-full"
      />

      <p class="font-mono text-xs break-all">
        {{ candidate.config.method }} {{ candidate.config.url }}
      </p>

      <ul
        v-if="candidate.warnings.length"
        class="list-disc space-y-2 pl-5 text-sm"
        aria-label="Import limitations"
      >
        <li
          v-for="(warning, index) in candidate.warnings"
          :key="index"
        >
          {{ warning }}
        </li>
      </ul>

      <UButton
        :disabled="disabled || busy"
        variant="outline"
        color="neutral"
        label="I reviewed the conversion. Apply request"
        @click="emit('apply', candidate.config)"
      />
    </template>
  </section>
</template>
