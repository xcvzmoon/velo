<script setup lang="ts">
  import type { Scenario } from '~/types/contracts';
  import { Field as FormischField, Form as FormischForm, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    seed: number;
    scenario: Scenario | null;
    disabled: boolean;
    reservedIds: string[];
  }>();
  const schema = v.object({
    seed: v.pipe(
      v.union([v.string(), v.number()]),
      v.toNumber(),
      v.integer(),
      v.minValue(0),
      v.maxValue(4_294_967_295),
    ),
    id: v.pipe(
      v.string(),
      v.regex(/^[A-Za-z0-9_-]{1,64}$/, 'Use 1 to 64 letters, digits, underscores or hyphens.'),
      v.check(
        (id) => !props.scenario || !props.reservedIds.includes(id),
        'Scenario names must be unique.',
      ),
    ),
    weight: v.pipe(
      v.union([v.string(), v.number()]),
      v.toNumber(),
      v.integer(),
      v.minValue(1),
      v.maxValue(10_000),
    ),
  });

  const emit = defineEmits<{
    apply: [value: v.InferOutput<typeof schema>];
    dirty: [value: boolean];
  }>();
  const formId = useId();
  const form = useForm({
    schema,
    initialInput: {
      seed: props.seed,
      id: props.scenario?.id ?? 'lifecycle',
      weight: props.scenario?.weight ?? 1,
    },
  });
  watch(
    () => getInput(form),
    () => emit('dirty', true),
    { deep: true },
  );
</script>

<template>
  <FormischForm
    :of="form"
    class="border-default space-y-4 rounded-md border p-4"
    @submit="
      emit('apply', $event);
      emit('dirty', false);
    "
  >
    <FormischField
      v-slot="field"
      :of="form"
      :path="['seed']"
    >
      <div class="space-y-2">
        <div class="grid min-w-0 gap-2">
          <label
            :for="`${formId}-seed`"
            class="text-sm font-medium"
          >
            Selection seed
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="`${formId}-seed`"
            :disabled="disabled"
            type="number"
            min="0"
            max="4294967295"
            step="1"
          />
        </div>

        <p
          v-if="field.errors"
          class="text-error text-xs"
          role="alert"
        >
          {{ field.errors.join(' ') }}
        </p>
      </div>
    </FormischField>

    <template v-if="scenario">
      <FormischField
        v-slot="field"
        :of="form"
        :path="['id']"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${formId}-id`"
              class="text-sm font-medium"
            >
              Scenario name / stable ID
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${formId}-id`"
              :disabled="disabled"
              autocomplete="off"
            />
          </div>

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['weight']"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${formId}-weight`"
              class="text-sm font-medium"
            >
              Relative selection weight
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${formId}-weight`"
              :disabled="disabled"
              type="number"
              min="1"
              max="10000"
              step="1"
            />
          </div>

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </FormischField>
    </template>

    <UButton
      :disabled="disabled"
      type="submit"
      variant="outline"
      color="neutral"
      label="Apply scenario settings"
    />
  </FormischForm>
</template>
