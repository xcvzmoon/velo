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
    <p class="text-muted-foreground text-sm">
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
              <Label :for="`${formId}-${item}-metric`">Metric</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
              >
                <SelectTrigger :id="`${formId}-${item}-metric`"><SelectValue /></SelectTrigger>

                <SelectContent>
                  <SelectItem
                    v-for="metric in metrics"
                    :key="metric.value"
                    :value="metric.value"
                  >
                    {{ metric.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['budgets', index, 'limit']"
          >
            <div class="space-y-2">
              <Label :for="`${formId}-${item}-limit`">Maximum worsening</Label>

              <Input
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
                class="text-destructive text-sm"
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
              <Label :for="`${formId}-${item}-unit`">Limit unit</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
              >
                <SelectTrigger :id="`${formId}-${item}-unit`"><SelectValue /></SelectTrigger>

                <SelectContent>
                  <SelectItem value="absolute">Absolute metric units</SelectItem>
                  <SelectItem value="percentage">Relative percent</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </FormischField>

          <Button
            :disabled="disabled"
            type="button"
            variant="ghost"
            @click="remove(form, { path: ['budgets'], at: index })"
          >
            Remove budget
          </Button>
        </div>

        <p
          v-if="array.errors"
          class="text-destructive text-sm"
          role="alert"
        >
          {{ array.errors.join(' ') }}
        </p>

        <Button
          :disabled="disabled || array.items.length >= 32"
          type="button"
          variant="outline"
          @click="
            insert(form, {
              path: ['budgets'],
              initialInput: { metric: 'response_p95_ms', limit: 10, unit: 'percentage' },
            })
          "
        >
          Add regression budget
        </Button>
      </div>
    </FormischFieldArray>

    <FormischField
      v-slot="field"
      :of="form"
      :path="['review']"
    >
      <div class="space-y-2">
        <Label :for="`${formId}-review`">Configuration review</Label>

        <Select
          v-model="field.input"
          :disabled="disabled"
        >
          <SelectTrigger :id="`${formId}-review`"><SelectValue /></SelectTrigger>

          <SelectContent>
            <SelectItem value="pending">Pending review</SelectItem>
            <SelectItem value="reviewed">
              I reviewed differences and redacted request context
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </FormischField>

    <Button
      :disabled="disabled"
      type="submit"
    >
      Save and evaluate budgets
    </Button>
  </FormischForm>
</template>
