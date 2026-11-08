<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    insert,
    remove,
  } from '@formisch/vue';

  const policyFields = [
    {
      name: 'window_ms',
      label: 'Observation window (ms)',
      min: 100,
      step: 100,
    },
    {
      name: 'min_samples',
      label: 'Minimum metric samples',
      min: 1,
      step: 1,
    },
    {
      name: 'sustained_ms',
      label: 'Sustained breach (ms)',
      min: 100,
      step: 100,
    },
  ] as const;

  defineProps<{ form: FormStore<typeof requestEditorSchema>; disabled: boolean }>();
</script>

<template>
  <div class="space-y-4 border-t pt-6">
    <h3 class="text-sm font-medium">Measurement window and automatic stopping</h3>

    <FormischField
      v-slot="field"
      :of="form"
      :path="['warmup_ms']"
    >
      <div class="max-w-xs space-y-2">
        <label
          class="text-sm font-medium"
          for="warmup-ms"
        >
          Warm-up within scheduling duration (ms)
        </label>

        <UInput
          v-model="field.input"
          v-bind="field.props"
          :disabled="disabled"
          :aria-invalid="Boolean(field.errors)"
          id="warmup-ms"
          aria-describedby="warmup-error"
          type="number"
          min="0"
          step="1"
        />

        <p
          v-if="field.errors"
          id="warmup-error"
          class="text-error text-xs"
          role="alert"
        >
          {{ field.errors[0] }}
        </p>
      </div>
    </FormischField>

    <p class="text-muted text-xs leading-relaxed">
      Warm-up is visible in full-run counters and charts. Requests admitted before its monotonic
      boundary stay excluded from evaluated summaries, even if they finish later. Evaluated requests
      remain included during drain. Scenario steps inherit the phase of their iteration admission.
      Final thresholds use this evaluated population.
    </p>

    <p class="text-muted text-xs leading-relaxed">
      Each policy describes an acceptable metric range. A sustained failure closes admission and
      follows the configured drain grace. Policies wait for warm-up, a full observation window and
      minimum metric samples. Windows use 100 ms resolution. Missing or invalid measurements cannot
      count as passing evidence. Request rates are HTTP requests/s, including scenario steps;
      workload arrival rates are iterations/s. Minimum samples count completed responses for
      latency, terminal requests for HTTP failure and request rate, or evaluated assertion checks.
      Live p95 uses two significant HDR digits; final response distributions use three.
    </p>

    <FormischFieldArray
      v-slot="fieldArray"
      :of="form"
      :path="['abort_policies']"
    >
      <div class="space-y-4">
        <UButton
          :disabled="disabled || fieldArray.items.length >= 8"
          type="button"
          variant="outline"
          size="sm"
          color="neutral"
          label="Add stop policy"
          @click="
            insert(form, {
              path: ['abort_policies'],
              initialInput: {
                rule: { metric: 'http_failure_percent', operator: 'less_than', limit: 50 },
                window_ms: 1000,
                min_samples: 10,
                sustained_ms: 2000,
              },
            })
          "
        />

        <p
          v-if="!fieldArray.items.length"
          class="text-muted text-xs"
        >
          No automatic stop policies configured.
        </p>

        <div
          v-for="(item, index) in fieldArray.items"
          :key="item"
          class="space-y-4 rounded-md border p-4"
        >
          <div class="flex items-center justify-between gap-3">
            <h4 class="text-sm font-medium">Stop policy {{ index + 1 }}</h4>

            <UButton
              :disabled="disabled"
              :aria-label="`Remove stop policy ${index + 1}`"
              type="button"
              variant="ghost"
              size="sm"
              color="neutral"
              label="Remove"
              @click="remove(form, { path: ['abort_policies'], at: index })"
            />
          </div>

          <div class="grid gap-4 sm:grid-cols-3">
            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'metric']"
            >
              <div class="space-y-2">
                <label
                  :for="`policy-${item}-metric`"
                  class="text-sm font-medium"
                >
                  Metric and unit
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :id="`policy-${item}-metric`"
                  :items="[
                    { label: 'Response p95 (ms)', value: 'response_p95_ms' },
                    { label: 'HTTP failure (%)', value: 'http_failure_percent' },
                    { label: 'Assertion failure (%)', value: 'assertion_failure_percent' },
                    { label: 'HTTP requests/s', value: 'request_rate' },
                  ]"
                  class="w-full"
                />
              </div>
            </FormischField>

            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'operator']"
            >
              <div class="space-y-2">
                <label
                  :for="`policy-${item}-operator`"
                  class="text-sm font-medium"
                >
                  Required comparison
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :id="`policy-${item}-operator`"
                  :items="[
                    { label: 'Strictly less than', value: 'less_than' },
                    { label: 'Strictly greater than', value: 'greater_than' },
                  ]"
                  class="w-full"
                />
              </div>
            </FormischField>

            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'limit']"
            >
              <div class="space-y-2">
                <label
                  :for="`policy-${item}-limit`"
                  class="text-sm font-medium"
                >
                  Limit
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`policy-${item}-limit`"
                  :disabled="disabled"
                  :aria-invalid="Boolean(field.errors)"
                  :aria-describedby="`policy-${item}-limit-error`"
                  type="number"
                  min="0"
                  step="any"
                />

                <p
                  v-if="field.errors"
                  :id="`policy-${item}-limit-error`"
                  class="text-error text-xs"
                  role="alert"
                >
                  {{ field.errors[0] }}
                </p>
              </div>
            </FormischField>

            <FormischField
              v-for="option in policyFields"
              v-slot="field"
              :key="option.name"
              :of="form"
              :path="['abort_policies', index, option.name]"
            >
              <div class="space-y-2">
                <label
                  :for="`policy-${item}-${option.name}`"
                  class="text-sm font-medium"
                >
                  {{ option.label }}
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`policy-${item}-${option.name}`"
                  :disabled="disabled"
                  :min="option.min"
                  :step="option.step"
                  :aria-invalid="Boolean(field.errors)"
                  :aria-describedby="`policy-${item}-${option.name}-error`"
                  type="number"
                />

                <p
                  v-if="field.errors"
                  :id="`policy-${item}-${option.name}-error`"
                  class="text-error text-xs"
                  role="alert"
                >
                  {{ field.errors[0] }}
                </p>
              </div>
            </FormischField>
          </div>
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
  </div>
</template>
