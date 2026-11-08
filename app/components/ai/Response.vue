<script setup lang="ts">
  import type { AiResult } from '~/types/ai';
  import type { TestDefinition } from '~/types/contracts';
  import { invoke } from '@tauri-apps/api/core';

  const props = defineProps<{
    result: AiResult;
    previous: TestDefinition | null;
    disabled: boolean;
    evidenceId: string;
  }>();
  const emit = defineEmits<{ apply: [definition: TestDefinition] }>();
  const error = shallowRef<string | null>(null);
  const applying = shallowRef<boolean>(false);
  const proposed = computed<TestDefinition | null>(() => {
    const definition = props.result.definition;
    if (!definition || !props.previous) {
      return definition;
    }
    return {
      ...definition,
      config: {
        ...definition.config,
        budget: props.previous.config.budget,
      },
    };
  });
  const source = computed<string>(() => JSON.stringify(proposed.value, null, 2));
  const notes = computed<{ title: string; entries: string[] }[]>(() => [
    {
      title: 'Assumptions',
      entries: props.result.assumptions,
    },
    {
      title: 'Missing inputs',
      entries: props.result.missing_inputs,
    },
    {
      title: 'Unsupported suggestions',
      entries: props.result.unsupported,
    },
    {
      title: 'Hypotheses, not established causes',
      entries: props.result.hypotheses,
    },
    {
      title: 'Suggested investigation',
      entries: props.result.next_steps,
    },
  ]);

  async function apply(text: string): Promise<void> {
    if (props.disabled || applying.value) {
      return;
    }
    applying.value = true;
    error.value = null;
    try {
      const definition = await invoke<TestDefinition>('parse_definition', { source: text });
      if (props.previous) {
        definition.config.budget = props.previous.config.budget;
      }
      if (definition.dataset) {
        parseDataDefinition(JSON.stringify(definition.dataset));
      }
      if (!props.disabled) {
        emit('apply', definition);
      }
    } catch (error_) {
      error.value = error_ instanceof Error ? error_.message : formatAuthoringError(error_);
    } finally {
      applying.value = false;
    }
  }
</script>

<template>
  <div
    class="space-y-5"
    aria-label="AI suggestion"
  >
    <p class="text-muted-foreground text-xs">
      {{ result.provider }} · {{ result.model }} ·
      {{ new Date(Number(result.generated_at) * 1000).toLocaleString() }}. Model output may vary.
      Target endpoints have not been verified.
    </p>

    <p class="text-muted-foreground font-mono text-xs break-all">
      Provider endpoint: {{ result.endpoint }}
    </p>

    <div
      v-if="result.observations.length"
      class="space-y-3"
    >
      <h3 class="text-sm font-medium">Observations with measurement references</h3>

      <ul class="space-y-3 text-sm">
        <li
          v-for="(observation, index) in result.observations"
          :key="index"
        >
          <p>{{ observation.text }}</p>

          <a
            v-for="reference in observation.evidence"
            :key="reference"
            :href="`#${evidenceId}`"
            class="mr-3 font-mono text-xs underline underline-offset-4"
          >
            {{ reference }}
          </a>
        </li>
      </ul>
    </div>

    <template
      v-for="{ title, entries } in notes"
      :key="title"
    >
      <div
        v-if="entries.length"
        class="space-y-2"
      >
        <h3 class="text-sm font-medium">{{ title }}</h3>

        <ul class="list-disc space-y-1 pl-5 text-sm">
          <li
            v-for="(entry, index) in entries"
            :key="index"
          >
            {{ entry }}
          </li>
        </ul>
      </div>
    </template>

    <template v-if="result.definition">
      <p class="text-muted-foreground text-sm">
        Review the current and proposed definitions below. Edit the proposal before applying it.
        Applying replaces the editor and preserves its current resource budgets. Dataset
        preparation, engine validation, load review, and target authorization are still required
        before execution.
      </p>

      <details
        v-if="previous"
        class="rounded-md border p-3"
      >
        <summary class="cursor-pointer text-sm">Current definition before replacement</summary>

        <pre class="mt-3 max-h-80 overflow-auto text-xs">{{
          JSON.stringify(previous, null, 2)
        }}</pre>
      </details>

      <AuthoringTextForm
        :source="source"
        :disabled="disabled || applying"
        label="Proposed definition (editable)"
        action="Apply reviewed definition to editor"
        @apply="apply"
      />
    </template>

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>
