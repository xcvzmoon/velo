<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    getInput,
    insert,
    remove,
    setInput,
  } from '@formisch/vue';

  const constantFields = [
    {
      name: 'vus',
      label: 'Virtual users',
    },
    {
      name: 'duration_ms',
      label: 'Scheduling duration (ms)',
    },
  ] as const;

  const arrivalFields = [
    {
      name: 'rate',
      label: 'Target iterations per second',
    },
    {
      name: 'capacity',
      label: 'Concurrent request capacity',
    },
    {
      name: 'duration_ms',
      label: 'Scheduling duration (ms)',
    },
  ] as const;

  const stageFields = [
    {
      name: 'duration_ms',
      label: 'Duration (ms)',
      min: 1,
    },
    {
      name: 'target_vus',
      label: 'Target VUs',
      min: 0,
    },
  ] as const;

  const props = defineProps<{ form: FormStore<typeof requestEditorSchema>; disabled: boolean }>();

  function applyPreset(preset: Parameters<typeof workloadPreset>[0]): void {
    const input = getInput(props.form);
    const budget = Number(input.max_vus);
    if (!Number.isInteger(budget) || budget < 1 || budget > 4_294_967_295) {
      return;
    }
    const { workload, warmup_ms, pacing_ms } = workloadPreset(preset, budget);
    setInput(props.form, {
      path: ['workloadKind'],
      input: workload.kind,
    });
    setInput(props.form, {
      path: ['vus'],
      input: workload.kind === 'constant' ? workload.vus : 1,
    });
    setInput(props.form, {
      path: ['duration_ms'],
      input: workload.kind === 'ramping' ? 10_000 : workload.duration_ms,
    });
    setInput(props.form, {
      path: ['start_vus'],
      input: workload.kind === 'ramping' ? workload.start_vus : 0,
    });
    setInput(props.form, {
      path: ['stages'],
      input: workload.kind === 'ramping' ? workload.stages : [],
    });
    setInput(props.form, {
      path: ['warmup_ms'],
      input: warmup_ms,
    });
    setInput(props.form, {
      path: ['pacing_ms'],
      input: pacing_ms,
    });
  }
</script>

<template>
  <div class="space-y-3">
    <h4 class="text-sm font-medium">Workload presets</h4>

    <div class="flex flex-wrap gap-2">
      <UButton
        v-for="preset in ['smoke', 'stress', 'spike', 'soak'] as const"
        :key="preset"
        :disabled="disabled"
        :label="capitalize(preset)"
        type="button"
        variant="outline"
        color="neutral"
        @click="applyPreset(preset)"
      />
    </div>

    <p class="text-muted text-xs leading-relaxed">
      Presets replace the schedule, warm-up and pacing with editable values. Peaks are capped at
      your worker budget. Budgets and stop policies stay unchanged. These names do not measure
      machine or target capacity.
    </p>
  </div>

  <FormischField
    v-slot="mode"
    :of="form"
    :path="['workloadKind']"
  >
    <div class="space-y-5">
      <div class="max-w-xs space-y-2">
        <div class="grid min-w-0 gap-2">
          <label
            class="text-sm font-medium"
            for="workload-kind"
          >
            Workload
          </label>

          <USelect
            v-model="mode.input"
            :disabled="disabled"
            :name="mode.props.name"
            :items="[
              { label: 'Constant VUs', value: 'constant' },
              { label: 'Ramping VUs', value: 'ramping' },
              { label: 'Constant arrival rate', value: 'arrival' },
            ]"
            id="workload-kind"
            class="w-full"
          />
        </div>
      </div>

      <div
        v-if="mode.input !== 'ramping'"
        class="grid gap-4 sm:grid-cols-2"
      >
        <FormischField
          v-for="option in mode.input === 'arrival' ? arrivalFields : constantFields"
          v-slot="field"
          :key="option.name"
          :of="form"
          :path="[option.name]"
        >
          <div class="space-y-2">
            <div class="grid min-w-0 gap-2">
              <label
                :for="`request-${option.name}`"
                class="text-sm font-medium"
              >
                {{ option.label }}
              </label>

              <UInput
                v-model="field.input"
                v-bind="field.props"
                :id="`request-${option.name}`"
                :aria-invalid="Boolean(field.errors)"
                :aria-describedby="`request-${option.name}-error`"
                type="number"
                min="1"
                step="1"
              />
            </div>

            <p
              v-if="field.errors"
              :id="`request-${option.name}-error`"
              class="text-error text-xs"
              role="alert"
            >
              {{ field.errors[0] }}
            </p>
          </div>
        </FormischField>
      </div>

      <p
        v-if="mode.input === 'arrival'"
        class="text-muted text-xs leading-relaxed"
      >
        Deadlines control admission; set iteration pacing to zero. Capacity limits concurrent
        requests. Expired deadlines and requests above capacity are dropped without a queue.
      </p>

      <div
        v-if="mode.input === 'ramping'"
        class="space-y-5"
      >
        <FormischField
          v-slot="field"
          :of="form"
          :path="['start_vus']"
        >
          <div class="max-w-xs space-y-2">
            <div class="grid min-w-0 gap-2">
              <label
                class="text-sm font-medium"
                for="start-vus"
              >
                Starting VUs
              </label>

              <UInput
                v-model="field.input"
                v-bind="field.props"
                :aria-invalid="Boolean(field.errors)"
                id="start-vus"
                aria-describedby="start-vus-error"
                type="number"
                min="0"
                step="1"
              />
            </div>

            <p
              v-if="field.errors"
              id="start-vus-error"
              class="text-error text-xs"
              role="alert"
            >
              {{ field.errors[0] }}
            </p>
          </div>
        </FormischField>

        <FormischFieldArray
          v-slot="fieldArray"
          :of="form"
          :path="['stages']"
        >
          <div class="space-y-4">
            <div class="flex items-center justify-between gap-3">
              <h4 class="text-sm font-medium">Ramp stages</h4>

              <UButton
                :disabled="disabled"
                type="button"
                variant="outline"
                color="neutral"
                label="Add stage"
                @click="
                  insert(form, {
                    path: ['stages'],
                    initialInput: { duration_ms: 10_000, target_vus: 0 },
                  })
                "
              />
            </div>

            <div
              v-for="(item, index) in fieldArray.items"
              :key="item"
              class="border-default grid items-start gap-3 rounded-md border p-4 sm:grid-cols-[auto_1fr_1fr_auto]"
            >
              <p class="text-muted pt-2 text-xs">{{ index + 1 }}</p>

              <FormischField
                v-for="{ name, label, min } in stageFields"
                v-slot="field"
                :key="name"
                :of="form"
                :path="['stages', index, name]"
              >
                <div class="space-y-2">
                  <div class="grid min-w-0 gap-2">
                    <label
                      :for="`stage-${item}-${name}`"
                      class="text-sm font-medium"
                    >
                      {{ label }}
                    </label>

                    <UInput
                      v-model="field.input"
                      v-bind="field.props"
                      :id="`stage-${item}-${name}`"
                      :min="min"
                      :aria-invalid="Boolean(field.errors)"
                      :aria-describedby="`stage-${item}-${name}-error`"
                      type="number"
                      step="1"
                    />
                  </div>

                  <p
                    v-if="field.errors"
                    :id="`stage-${item}-${name}-error`"
                    class="text-error text-xs"
                    role="alert"
                  >
                    {{ field.errors[0] }}
                  </p>
                </div>
              </FormischField>

              <UButton
                :disabled="disabled"
                :aria-label="`Remove stage ${index + 1}`"
                type="button"
                variant="ghost"
                class="sm:mt-6"
                color="neutral"
                label="Remove"
                @click="remove(form, { path: ['stages'], at: index })"
              />
            </div>

            <p
              v-if="fieldArray.errors"
              class="text-error text-xs"
              role="alert"
            >
              {{ fieldArray.errors[0] }}
            </p>
          </div>
        </FormischFieldArray>

        <p class="text-muted text-xs leading-relaxed">
          Each stage ramps linearly from the previous count. Use the same target to hold a level.
          Zero VUs pauses new requests. Retiring users finish their current request and exit; they
          still occupy worker slots until cleanup finishes.
        </p>
      </div>
    </div>
  </FormischField>
</template>
