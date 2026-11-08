<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    insert,
    remove,
  } from '@formisch/vue';

  defineProps<{ form: FormStore<typeof requestEditorSchema>; disabled: boolean }>();
</script>

<template>
  <div class="space-y-4 border-t pt-6">
    <h3 class="text-sm font-medium">Final thresholds</h3>

    <p class="text-muted text-xs leading-relaxed">
      Evaluate the measurement population after cleanup, excluding requests admitted during warm-up.
      Response p95 covers completed HTTP responses of all statuses. HTTP failure percentage covers
      status, transport, and timeout failures over non-cancelled terminal attempts. Request rate
      uses those attempts per evaluated second, including drain. Assertion failure percentage counts
      failed checks over evaluated checks; any unavailable checks make that threshold insufficient.
      Equality fails. Missing or invalid required measurements cannot pass. Partial runs use
      observed work.
    </p>

    <FormischFieldArray
      v-slot="fieldArray"
      :of="form"
      :path="['thresholds']"
    >
      <div class="space-y-4">
        <UButton
          :disabled="disabled"
          type="button"
          size="sm"
          variant="outline"
          color="neutral"
          label="Add threshold"
          @click="
            insert(form, {
              path: ['thresholds'],
              initialInput: {
                metric: 'response_p95_ms',
                operator: 'less_than',
                limit: 500,
              },
            })
          "
        />

        <p
          v-if="!fieldArray.items.length"
          class="text-muted text-xs"
        >
          No thresholds configured.
        </p>

        <div
          v-for="(item, index) in fieldArray.items"
          :key="item"
          class="grid items-start gap-3 sm:grid-cols-[1fr_1fr_1fr_auto]"
        >
          <FormischField
            v-slot="field"
            :of="form"
            :path="['thresholds', index, 'metric']"
          >
            <div class="space-y-2">
              <label
                :for="`threshold-${item}-metric`"
                class="text-sm font-medium"
              >
                Metric and unit
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
                :id="`threshold-${item}-metric`"
                :items="[
                  { label: 'Response p95 (ms)', value: 'response_p95_ms' },
                  { label: 'HTTP failure (%)', value: 'http_failure_percent' },
                  { label: 'Assertion failure (%)', value: 'assertion_failure_percent' },
                  { label: 'Requests/s', value: 'request_rate' },
                ]"
                class="w-full"
              />
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['thresholds', index, 'operator']"
          >
            <div class="space-y-2">
              <label
                :for="`threshold-${item}-operator`"
                class="text-sm font-medium"
              >
                Comparison
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
                :id="`threshold-${item}-operator`"
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
            :path="['thresholds', index, 'limit']"
          >
            <div class="space-y-2">
              <label
                :for="`threshold-${item}-limit`"
                class="text-sm font-medium"
              >
                Limit
              </label>

              <UInput
                v-model="field.input"
                v-bind="field.props"
                :id="`threshold-${item}-limit`"
                :aria-invalid="Boolean(field.errors)"
                :aria-describedby="`threshold-${item}-error`"
                type="number"
                min="0"
                step="any"
              />

              <p
                v-if="field.errors"
                :id="`threshold-${item}-error`"
                class="text-error text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>

          <UButton
            :disabled="disabled"
            :aria-label="`Remove threshold ${index + 1}`"
            type="button"
            variant="ghost"
            class="sm:mt-6"
            color="neutral"
            label="Remove"
            @click="remove(form, { path: ['thresholds'], at: index })"
          />
        </div>
      </div>
    </FormischFieldArray>
  </div>
</template>
