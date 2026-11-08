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

    <p class="text-muted-foreground text-xs leading-relaxed">
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
        <Button
          :disabled="disabled"
          type="button"
          size="sm"
          variant="outline"
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
        >
          Add threshold
        </Button>

        <p
          v-if="!fieldArray.items.length"
          class="text-muted-foreground text-xs"
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
              <Label :for="`threshold-${item}-metric`">Metric and unit</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
              >
                <SelectTrigger
                  :id="`threshold-${item}-metric`"
                  class="w-full"
                >
                  <SelectValue />
                </SelectTrigger>

                <SelectContent>
                  <SelectItem value="response_p95_ms">Response p95 (ms)</SelectItem>
                  <SelectItem value="http_failure_percent">HTTP failure (%)</SelectItem>
                  <SelectItem value="assertion_failure_percent">Assertion failure (%)</SelectItem>
                  <SelectItem value="request_rate">Requests/s</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['thresholds', index, 'operator']"
          >
            <div class="space-y-2">
              <Label :for="`threshold-${item}-operator`">Comparison</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
              >
                <SelectTrigger
                  :id="`threshold-${item}-operator`"
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
            :path="['thresholds', index, 'limit']"
          >
            <div class="space-y-2">
              <Label :for="`threshold-${item}-limit`">Limit</Label>

              <Input
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
                class="text-destructive text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>

          <Button
            :disabled="disabled"
            :aria-label="`Remove threshold ${index + 1}`"
            type="button"
            variant="ghost"
            class="sm:mt-6"
            @click="remove(form, { path: ['thresholds'], at: index })"
          >
            Remove
          </Button>
        </div>
      </div>
    </FormischFieldArray>
  </div>
</template>
