<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { RunConfig, RunConfirmation, TestDefinition } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { confirmationSettingsSchema } from '~/schemas/run';

  definePageMeta({ keepalive: true });

  const { $runs } = useNuxtApp();
  const { active, busy, error } = $runs;

  const confirmationForm = useForm({
    schema: confirmationSettingsSchema,
    initialInput: { updateIntervalMs: '1000' },
  });

  const apiDefinition = useState<TestDefinition | null>('api-load-definition', () => null);
  const loaded = shallowRef<TestDefinition | null>(null);

  const confirmation = shallowRef<RunConfirmation | null>(null);
  const prepared = shallowRef<RunConfig | null>(null);

  watch(
    apiDefinition,
    (definition) => {
      if (definition) {
        loaded.value = definition;
        apiDefinition.value = null;
        cancel();
      }
    },
    { immediate: true },
  );

  async function prepare(config: RunConfig): Promise<void> {
    confirmation.value = await $runs.prepare(config);
    prepared.value = confirmation.value ? config : null;
  }

  async function start(settings: InferOutput<typeof confirmationSettingsSchema>): Promise<void> {
    const config = prepared.value;
    if (!config) {
      return;
    }

    await $runs.start(config, settings.updateIntervalMs);
    confirmation.value = null;
    prepared.value = null;
  }

  function cancel(): void {
    confirmation.value = null;
    prepared.value = null;
  }
</script>

<template>
  <div class="space-y-8">
    <ShellPageHeading
      title="Test studio"
      description="Build a test, prepare its data, and review the requested load before execution."
    />

    <RunMonitor v-if="active" />

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <FormischForm
      v-if="confirmation"
      :of="confirmationForm"
      class="bg-primary/5 border-primary/30 space-y-5 rounded-lg border p-6"
      aria-labelledby="confirmation-heading"
      @submit="start"
    >
      <div>
        <h2
          id="confirmation-heading"
          class="text-lg font-medium"
        >
          Review requested load
        </h2>

        <p class="text-muted mt-2 text-sm">{{ confirmation.origin }}</p>
      </div>

      <dl class="grid gap-4 text-sm sm:grid-cols-2">
        <div>
          <dt class="text-muted">Workload</dt>

          <dd class="mt-1 font-mono">
            <template v-if="confirmation.workload.kind === 'arrival'">
              {{ confirmation.workload.rate }} iterations/s, capacity {{ confirmation.vus }} for
              {{ confirmation.duration_ms / 1000 }} s
            </template>

            <template v-else>
              {{ confirmation.workload.kind === 'ramping' ? 'Ramping, peak' : 'Constant' }}
              {{ confirmation.vus }} VUs for {{ confirmation.duration_ms / 1000 }} s
            </template>
          </dd>
        </div>

        <div>
          <dt class="text-muted">Local resource budgets</dt>

          <dd class="mt-1 font-mono">
            {{ confirmation.budget.max_vus }} tasks /
            {{ confirmation.budget.max_request_bytes }} request bytes
          </dd>
        </div>
      </dl>

      <p class="text-muted text-sm">
        Warm-up {{ confirmation.warmup_ms / 1000 }} s within the schedule; evaluated admission
        window {{ (confirmation.duration_ms - confirmation.warmup_ms) / 1000 }} s.
        {{ confirmation.abort_policies.length }} automatic stop policies configured. In-flight
        evaluated requests remain included during drain.
      </p>

      <p
        v-if="confirmation.dataset"
        class="text-muted text-sm"
      >
        Dataset: {{ confirmation.dataset.records }} records from {{ confirmation.dataset.source }}.
        Lifetime {{ confirmation.dataset.lifetime }}, allocation
        {{ confirmation.dataset.allocation }}, exhaustion {{ confirmation.dataset.exhaustion }},
        seed {{ confirmation.dataset.seed }}.
      </p>

      <div
        v-if="confirmation.workload.kind === 'ramping'"
        class="space-y-3 rounded-md border p-4 text-sm"
      >
        <p>Start at {{ confirmation.workload.start_vus }} VUs.</p>

        <ol class="space-y-2">
          <li
            v-for="(stage, index) in confirmation.workload.stages"
            :key="index"
            class="font-mono text-xs"
          >
            {{ index + 1 }}. Ramp to {{ stage.target_vus }} VUs over
            {{ stage.duration_ms / 1000 }} s
          </li>
        </ol>

        <p class="text-muted text-xs">
          Retiring users finish their current request. Actual VUs can lag the desired count during
          transitions.
        </p>
      </div>

      <p
        v-if="confirmation.warnings.length"
        class="text-error text-sm"
        role="alert"
      >
        {{ confirmation.warnings.join(', ') }}: This load may saturate the generator or target.
        Review your local budgets before starting.
      </p>

      <p class="text-muted text-sm leading-relaxed">
        Starting acknowledges that you are authorized to load this target at the requested level.
        Velo does not verify ownership. Automatic retries are disabled. Configured redirects stay on
        the target origin. System certificate validation is enabled.
      </p>

      <FormischField
        v-slot="field"
        :of="confirmationForm"
        :path="['updateIntervalMs']"
      >
        <div class="max-w-xs space-y-2">
          <label
            class="text-sm font-medium"
            for="update-interval"
          >
            Live update interval
          </label>

          <USelect
            v-model="field.input"
            :disabled="busy"
            :name="field.props.name"
            :items="[
              { label: '500 milliseconds', value: '500' },
              { label: '1 second', value: '1000' },
              { label: '2 seconds', value: '2000' },
            ]"
            id="update-interval"
            class="w-full"
          />
        </div>
      </FormischField>

      <div class="flex flex-wrap gap-3">
        <UButton
          :disabled="busy || active"
          type="submit"
          label="I am authorized. Start run"
        />

        <UButton
          :disabled="busy"
          type="button"
          variant="outline"
          color="neutral"
          label="Back to editor"
          @click="cancel"
        />
      </div>
    </FormischForm>

    <details class="rounded-lg border p-4">
      <summary class="text-muted cursor-pointer text-sm font-medium">
        Saved request configurations
      </summary>

      <RunSavedConfigurations
        :disabled="active || busy"
        @load="
          loaded = $event;
          cancel();
        "
      />
    </details>

    <AuthoringEditor
      :loaded="loaded"
      :disabled="active || busy"
      @prepare="prepare"
      @invalidate="cancel"
    />

    <p class="text-muted text-xs">
      Navigation preserves the active run. History is stored locally; storage failures appear in
      Results. Loading a configuration requires reviewing and authorizing it again.
    </p>

    <UButton
      v-if="!active && $runs.snapshot.value"
      variant="outline"
      to="/results"
      color="neutral"
      label="Inspect latest results"
    />
  </div>
</template>
