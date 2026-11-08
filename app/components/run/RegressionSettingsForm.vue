<script setup lang="ts">
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    Form as FormischForm,
    insert,
    remove,
    getInput,
    useForm,
  } from '@formisch/vue';
  import * as v from 'valibot';

  const metrics = [
    {
      value: 'response_p50_ms',
      label: 'Response p50 (ms)',
    },
    {
      value: 'response_p95_ms',
      label: 'Response p95 (ms)',
    },
    {
      value: 'response_p99_ms',
      label: 'Response p99 (ms)',
    },
    {
      value: 'request_rate',
      label: 'Throughput (requests/s)',
    },
    {
      value: 'http_failure_percent',
      label: 'HTTP failure (percentage points)',
    },
    {
      value: 'assertion_failure_percent',
      label: 'Assertion failure (percentage points)',
    },
  ] as const;
  const schema = v.object({
    review: v.picklist(['pending', 'reviewed']),
    budgets: v.pipe(
      v.array(
        v.object({
          metric: v.picklist([
            'response_p50_ms',
            'response_p95_ms',
            'response_p99_ms',
            'request_rate',
            'http_failure_percent',
            'assertion_failure_percent',
          ]),
          limit: v.pipe(
            v.union([v.string(), v.number()]),
            v.toNumber(),
            v.finite(),
            v.minValue(0),
            v.maxValue(1e12),
          ),
          unit: v.picklist(['absolute', 'percentage']),
        }),
      ),
      v.maxLength(32),
    ),
  });
  const props = defineProps<{ settings: ComparisonSettings; disabled: boolean }>();
  const emit = defineEmits<{ save: [settings: ComparisonSettings]; dirty: [] }>();
  const formId = useId();
  const form = useForm({
    schema,
    initialInput: {
      review: props.settings.configuration_reviewed ? 'reviewed' : 'pending',
      budgets: props.settings.budgets.map(({ metric, limit, percentage }) => ({
        metric,
        limit,
        unit: percentage ? ('percentage' as const) : ('absolute' as const),
      })),
    },
  });

  function save(output: v.InferOutput<typeof schema>): void {
    emit('save', {
      baseline_id: props.settings.baseline_id,
      configuration_reviewed: output.review === 'reviewed',
      budgets: output.budgets.map(({ metric, limit, unit }) => ({
        metric,
        limit,
        percentage: unit === 'percentage',
      })),
    });
  }
  watch(
    () => getInput(form),
    () => emit('dirty'),
    { deep: true },
  );
</script>

<template>
  <FormischForm
    :of="form"
    class="space-y-4"
    @submit="save"
  >
    <p class="text-muted text-sm">
      Limits bound worsening. Throughput decreases count as worsening; latency and failure increases
      count as worsening. Equality passes. Relative percent is unavailable for a zero baseline.
    </p>

    <FormischFieldArray
      v-slot="array"
      :of="form"
      :path="['budgets']"
    >
      <div class="space-y-3">
        <div
          v-for="(item, index) in array.items"
          :key="item"
          class="grid gap-3 rounded-md border p-3 md:grid-cols-4"
        >
          <FormischField
            v-slot="field"
            :of="form"
            :path="['budgets', index, 'metric']"
          >
            <div class="space-y-2">
              <label
                :for="`${formId}-${item}-metric`"
                class="text-sm font-medium"
              >
                Metric
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :id="`${formId}-${item}-metric`"
                :items="[
                  ...metrics.map((metric) => ({ label: metric.label, value: metric.value })),
                ]"
                class="w-full"
              />
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['budgets', index, 'limit']"
          >
            <div class="space-y-2">
              <label
                :for="`${formId}-${item}-limit`"
                class="text-sm font-medium"
              >
                Maximum worsening
              </label>

              <UInput
                v-model="field.input"
                v-bind="field.props"
                :id="`${formId}-${item}-limit`"
                :disabled="disabled"
                type="number"
                min="0"
                max="1000000000000"
                step="any"
              />

              <p
                v-if="field.errors"
                class="text-error text-sm"
                role="alert"
              >
                {{ field.errors.join(' ') }}
              </p>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['budgets', index, 'unit']"
          >
            <div class="space-y-2">
              <label
                :for="`${formId}-${item}-unit`"
                class="text-sm font-medium"
              >
                Limit unit
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :id="`${formId}-${item}-unit`"
                :items="[
                  { label: 'Absolute metric units', value: 'absolute' },
                  { label: 'Relative percent', value: 'percentage' },
                ]"
                class="w-full"
              />
            </div>
          </FormischField>

          <UButton
            :disabled="disabled"
            type="button"
            variant="ghost"
            color="neutral"
            label="Remove budget"
            @click="remove(form, { path: ['budgets'], at: index })"
          />
        </div>

        <p
          v-if="array.errors"
          class="text-error text-sm"
          role="alert"
        >
          {{ array.errors.join(' ') }}
        </p>

        <UButton
          :disabled="disabled || array.items.length >= 32"
          type="button"
          variant="outline"
          color="neutral"
          label="Add regression budget"
          @click="
            insert(form, {
              path: ['budgets'],
              initialInput: { metric: 'response_p95_ms', limit: 10, unit: 'percentage' },
            })
          "
        />
      </div>
    </FormischFieldArray>

    <FormischField
      v-slot="field"
      :of="form"
      :path="['review']"
    >
      <div class="space-y-2">
        <label
          :for="`${formId}-review`"
          class="text-sm font-medium"
        >
          Configuration review
        </label>

        <USelect
          v-model="field.input"
          :disabled="disabled"
          :id="`${formId}-review`"
          :items="[
            { label: 'Pending review', value: 'pending' },
            { label: 'I reviewed differences and redacted request context', value: 'reviewed' },
          ]"
          class="w-full"
        />
      </div>
    </FormischField>

    <UButton
      :disabled="disabled"
      type="submit"
      label="Save and evaluate budgets"
    />
  </FormischForm>
</template>
