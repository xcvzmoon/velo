<script setup lang="ts">
  import type { ResponseDiagnostics } from '~/types/contracts';

  defineProps<{ diagnostics: ResponseDiagnostics }>();
</script>

<template>
  <section
    class="space-y-4 rounded-md border p-4"
    aria-label="Response assertions and failure diagnostics"
  >
    <h3 class="text-sm font-medium">Response assertions and failure diagnostics</h3>

    <p class="text-muted text-xs">
      Checks stay separate from HTTP outcomes. Per assertion, passed + failed + unavailable equals
      terminal attempts, including cancellations. Invalid JSON, unsupported encoding, unreadable
      bodies and truncation cannot pass. Multiple failed assertions count separately.
    </p>

    <div
      v-if="diagnostics.assertions.length"
      class="overflow-x-auto"
    >
      <UTable
        :data="diagnostics.assertions"
        :columns="[
          { id: 'assertion', header: 'Assertion' },
          { id: 'passed', header: 'Passed' },
          { id: 'failed', header: 'Failed' },
          { id: 'unavailable', header: 'Unavailable / error' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption><span class="sr-only">Per-assertion outcomes</span></template>

        <template #assertion-cell="{ row }">
          <span class="font-normal">{{ row.original.id }}</span>
        </template>

        <template #passed-cell="{ row }">
          <span class="font-mono">{{ row.original.passed }}</span>
        </template>

        <template #failed-cell="{ row }">
          <span class="font-mono">{{ row.original.failed }}</span>
        </template>

        <template #unavailable-cell="{ row }">
          <span class="font-mono">{{ row.original.unavailable }}</span>
        </template>
      </UTable>
    </div>

    <div
      v-if="diagnostics.groups.length"
      class="max-h-80 overflow-auto"
    >
      <UTable
        :data="diagnostics.groups"
        :columns="[
          { id: 'assertion', header: 'Assertion' },
          { id: 'cause', header: 'Cause' },
          { id: 'status', header: 'HTTP' },
          { id: 'count', header: 'Count' },
        ]"
        :ui="{ td: 'text-xs font-mono', th: 'text-xs' }"
        tabindex="0"
        class="w-full"
      >
        <template #caption><span class="sr-only">Grouped failure causes</span></template>

        <template #assertion-cell="{ row }">
          <span class="font-normal">
            {{
              row.original.example.assertion === null
                ? 'Request'
                : diagnostics.assertions[row.original.example.assertion]?.id
            }}
          </span>
        </template>

        <template #cause-cell="{ row }">
          <span>{{ row.original.example.cause.replaceAll('_', ' ') }}</span>
        </template>

        <template #status-cell="{ row }">
          <span class="font-mono">{{ row.original.example.status ?? 'No response' }}</span>
        </template>

        <template #count-cell="{ row }">
          <span class="font-mono">{{ row.original.count }}</span>
        </template>
      </UTable>
    </div>

    <p
      v-else
      class="text-muted text-xs"
    >
      No failure causes observed.
    </p>

    <details
      v-if="diagnostics.samples.length"
      class="text-xs"
    >
      <summary class="cursor-pointer">
        {{ diagnostics.samples.length }} redacted representative samples
      </summary>

      <ul class="mt-3 space-y-2">
        <li
          v-for="(sample, index) in diagnostics.samples"
          :key="index"
          class="rounded border p-2"
        >
          {{ sample.assertion === null ? 'Request' : diagnostics.assertions[sample.assertion]?.id }}
          · HTTP {{ sample.status ?? 'unavailable' }} · {{ sample.cause.replaceAll('_', ' ') }}.
          Response content withheld.
        </li>
      </ul>
    </details>

    <p class="text-muted text-xs leading-relaxed">
      Inspection cap: {{ diagnostics.limits.body_bytes }} bytes per worker. Truncated responses:
      {{ diagnostics.truncated_responses }}. Samples: {{ diagnostics.samples.length }} /
      {{ diagnostics.limits.sample_count }}; {{ diagnostics.retained_bytes }} /
      {{ diagnostics.limits.sample_bytes }} serialized bytes. Dropped representatives:
      {{ diagnostics.dropped_samples }}; ungrouped occurrences: {{ diagnostics.dropped_groups }}.
      Samples contain status and cause only, with response bodies, headers and expected values
      withheld before IPC or storage. Deleting a run removes its samples.
    </p>
  </section>
</template>
