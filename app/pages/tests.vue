<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { RunConfig, RunConfirmation } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { confirmationSettingsSchema } from '~/schemas/run';

  const { $runs } = useNuxtApp();
  const { active, busy, error } = $runs;

  const confirmationForm = useForm({
    schema: confirmationSettingsSchema,
    initialInput: { updateIntervalMs: '1000' },
  });

  const confirmation = shallowRef<RunConfirmation | null>(null);
  const prepared = shallowRef<RunConfig | null>(null);

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
    if (active.value) {
      confirmation.value = null;
      prepared.value = null;
    }
  }

  function cancel(): void {
    confirmation.value = null;
    prepared.value = null;
  }
</script>

<template>
  <div class="space-y-8">
    <ShellPageHeading
      title="Tests"
      description="Configure a request, review the load, and monitor your API."
    />

    <RunMonitor v-if="active" />

    <p
      v-if="error"
      class="text-destructive text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <FormischForm
      v-if="confirmation"
      :of="confirmationForm"
      class="space-y-5 rounded-lg border p-6"
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

        <p class="text-muted-foreground mt-2 text-sm">{{ confirmation.origin }}</p>
      </div>

      <dl class="grid gap-4 text-sm sm:grid-cols-2">
        <div>
          <dt class="text-muted-foreground">Workload</dt>

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
          <dt class="text-muted-foreground">Local resource budgets</dt>

          <dd class="mt-1 font-mono">
            {{ confirmation.budget.max_vus }} tasks /
            {{ confirmation.budget.max_request_bytes }} request bytes
          </dd>
        </div>
      </dl>

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

        <p class="text-muted-foreground text-xs">
          Retiring users finish their current request. Actual VUs can lag the desired count during
          transitions.
        </p>
      </div>

      <p
        v-if="confirmation.warnings.length"
        class="text-destructive text-sm"
        role="alert"
      >
        {{ confirmation.warnings.join(', ') }}: This load may saturate the generator or target.
        Review your local budgets before starting.
      </p>

      <p class="text-muted-foreground text-sm leading-relaxed">
        Starting acknowledges that you are authorized to load this target at the requested level.
        Velo does not verify ownership. Redirects and retries are disabled. System certificate
        validation is enabled.
      </p>

      <FormischField
        v-slot="field"
        :of="confirmationForm"
        :path="['updateIntervalMs']"
      >
        <div class="max-w-xs space-y-2">
          <Label for="update-interval">Live update interval</Label>

          <Select
            v-model="field.input"
            :disabled="busy"
            :name="field.props.name"
          >
            <SelectTrigger
              id="update-interval"
              class="w-full"
            >
              <SelectValue />
            </SelectTrigger>

            <SelectContent>
              <SelectItem value="500">500 milliseconds</SelectItem>
              <SelectItem value="1000">1 second</SelectItem>
              <SelectItem value="2000">2 seconds</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </FormischField>

      <div class="flex flex-wrap gap-3">
        <Button
          :disabled="busy || active"
          type="submit"
        >
          I am authorized. Start run
        </Button>

        <Button
          :disabled="busy"
          type="button"
          variant="outline"
          @click="cancel"
        >
          Back to editor
        </Button>
      </div>
    </FormischForm>

    <RunRequestEditor
      :disabled="active || busy || confirmation !== null"
      @prepare="prepare"
    />

    <p class="text-muted-foreground text-xs">
      Navigation preserves the active run. Request settings and results are held in memory; saved
      history is not available.
    </p>

    <Button
      v-if="!active && $runs.snapshot.value"
      variant="outline"
      as-child
    >
      <NuxtLink to="/results">Inspect latest results</NuxtLink>
    </Button>
  </div>
</template>
