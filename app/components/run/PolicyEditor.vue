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
        <Label for="warmup-ms">Warm-up within scheduling duration (ms)</Label>

        <Input
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
          class="text-destructive text-xs"
          role="alert"
        >
          {{ field.errors[0] }}
        </p>
      </div>
    </FormischField>

    <p class="text-muted-foreground text-xs leading-relaxed">
      Warm-up is visible in full-run counters and charts. Requests admitted before its monotonic
      boundary stay excluded from evaluated summaries, even if they finish later. Evaluated requests
      remain included during drain. Scenario steps inherit the phase of their iteration admission.
      Final thresholds use this evaluated population.
    </p>

    <p class="text-muted-foreground text-xs leading-relaxed">
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
        <Button
          :disabled="disabled || fieldArray.items.length >= 8"
          type="button"
          variant="outline"
          size="sm"
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
        >
          Add stop policy
        </Button>

        <p
          v-if="!fieldArray.items.length"
          class="text-muted-foreground text-xs"
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

            <Button
              :disabled="disabled"
              :aria-label="`Remove stop policy ${index + 1}`"
              type="button"
              variant="ghost"
              size="sm"
              @click="remove(form, { path: ['abort_policies'], at: index })"
            >
              Remove
            </Button>
          </div>

          <div class="grid gap-4 sm:grid-cols-3">
            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'metric']"
            >
              <div class="space-y-2">
                <Label :for="`policy-${item}-metric`">Metric and unit</Label>

                <Select
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                >
                  <SelectTrigger
                    :id="`policy-${item}-metric`"
                    class="w-full"
                  >
                    <SelectValue />
                  </SelectTrigger>

                  <SelectContent>
                    <SelectItem value="response_p95_ms">Response p95 (ms)</SelectItem>
                    <SelectItem value="http_failure_percent">HTTP failure (%)</SelectItem>
                    <SelectItem value="assertion_failure_percent">Assertion failure (%)</SelectItem>
                    <SelectItem value="request_rate">HTTP requests/s</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </FormischField>

            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'operator']"
            >
              <div class="space-y-2">
                <Label :for="`policy-${item}-operator`">Required comparison</Label>

                <Select
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                >
                  <SelectTrigger
                    :id="`policy-${item}-operator`"
                    class="w-full"
                  >
                    <SelectValue />
                  </SelectTrigger>

                  <SelectContent>
                    <SelectItem value="less_than">Strictly less than</SelectItem>
                    <SelectItem value="greater_than">Strictly greater than</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </FormischField>

            <FormischField
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, 'rule', 'limit']"
            >
              <div class="space-y-2">
                <Label :for="`policy-${item}-limit`">Limit</Label>

                <Input
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
                  class="text-destructive text-xs"
                  role="alert"
                >
                  {{ field.errors[0] }}
                </p>
              </div>
            </FormischField>

            <FormischField
              v-for="option in policyFields"
              :key="option.name"
              v-slot="field"
              :of="form"
              :path="['abort_policies', index, option.name]"
            >
              <div class="space-y-2">
                <Label :for="`policy-${item}-${option.name}`">{{ option.label }}</Label>

                <Input
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
                  class="text-destructive text-xs"
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
          class="text-destructive text-xs"
          role="alert"
        >
          {{ fieldArray.errors[0] }}
        </p>
      </div>
    </FormischFieldArray>
  </div>
</template>
