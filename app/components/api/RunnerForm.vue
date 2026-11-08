<script setup lang="ts">
  import { Field, Form, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    environments: string[];
    disabled: boolean;
    selection: string;
    hasRequest: boolean;
    preparedRows: Record<string, string>[];
  }>();
  const emit = defineEmits<{
    run: [
      settings: {
        environment: string | null;
        iterations: number;
        delayMs: number;
        stopOnFailure: boolean;
        rows: Record<string, string>[];
      },
      interactive: boolean,
    ];
    environment: [value: string | null];
  }>();
  const numberSchema = v.pipe(
    v.union([v.number(), v.string()]),
    v.toNumber(),
    v.integer(),
    v.minValue(0),
  );
  const schema = v.object({
    environment: v.string(),
    iterations: v.pipe(numberSchema, v.minValue(1), v.maxValue(100)),
    delayMs: v.pipe(numberSchema, v.maxValue(60000)),
    stopOnFailure: v.boolean(),
    authorized: v.pipe(
      v.boolean(),
      v.check((value) => value, 'Confirm authorization before sending.'),
    ),
    rows: v.pipe(
      v.string(),
      v.parseJson(),
      v.array(v.record(v.string(), v.string())),
      v.maxLength(10000),
    ),
  });
  const form = useForm({
    schema,
    initialInput: {
      environment: '__shared__',
      iterations: 1,
      delayMs: 0,
      stopOnFailure: true,
      authorized: false,
      rows: '[]',
    },
  });
  const interactive = shallowRef<boolean>(false);
  function submit(settings: v.InferOutput<typeof schema>): void {
    emit(
      'run',
      {
        environment: settings.environment === '__shared__' ? null : settings.environment,
        iterations: interactive.value ? 1 : settings.iterations,
        delayMs: interactive.value ? 0 : settings.delayMs,
        stopOnFailure: settings.stopOnFailure,
        rows: props.preparedRows.length ? props.preparedRows : settings.rows,
      },
      interactive.value,
    );
  }
</script>

<template>
  <Form
    :of="form"
    class="space-y-4 rounded-lg border p-5"
    @submit="submit"
  >
    <h2 class="font-medium">Send or run ordered functional checks</h2>

    <p class="text-muted text-sm">
      Run scope: {{ selection }}. Requests execute sequentially, with no automatic retries. Sending
      uses the current collection/environment session. Collection runs start with a fresh session.
    </p>

    <div class="grid gap-3 sm:grid-cols-3">
      <Field
        v-slot="field"
        :of="form"
        :path="['environment']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="api-environment"
          >
            Environment
          </label>

          <USelect
            v-model="field.input"
            :disabled="disabled"
            :items="[
              { label: 'Shared variables', value: '__shared__' },
              ...environments.map((name) => ({ label: name, value: name })),
            ]"
            id="api-environment"
            class="w-full"
            @update:model-value="
              emit('environment', String($event) === '__shared__' ? null : String($event))
            "
          />
        </div>
      </Field>

      <Field
        v-for="name in ['iterations', 'delayMs'] as const"
        v-slot="field"
        :key="name"
        :of="form"
        :path="[name]"
      >
        <div class="space-y-2">
          <label
            :for="`api-${name}`"
            class="text-sm font-medium"
          >
            {{ name === 'iterations' ? 'Iterations' : 'Delay between requests (ms)' }}
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="`api-${name}`"
            :disabled="disabled"
            type="number"
            min="0"
          />

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </Field>
    </div>

    <Field
      v-slot="field"
      :of="form"
      :path="['stopOnFailure']"
    >
      <div class="flex items-center gap-2">
        <UCheckbox
          :model-value="field.input"
          :disabled="disabled"
          id="api-stop-failure"
          @update:model-value="field.input = $event === true"
        />

        <label
          class="text-sm font-medium"
          for="api-stop-failure"
        >
          Stop on first failure
        </label>
      </div>
    </Field>

    <details>
      <summary class="cursor-pointer text-sm">Prepared dataset rows</summary>

      <p class="text-muted my-2 text-xs">
        Optional JSON array of prepared scalar string records. Configure the shared dataset recipe
        in the request definition. CSV/JSON and Faker preparation uses the same recipe as Tests.
      </p>

      <Field
        v-slot="field"
        :of="form"
        :path="['rows']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="api-prepared-rows"
          >
            Prepared rows (JSON)
          </label>

          <UTextarea
            v-model="field.input"
            v-bind="field.props"
            :disabled="disabled"
            :ui="{ base: 'font-mono text-xs md:text-xs' }"
            id="api-prepared-rows"
          />

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors.join(' ') }}
          </p>
        </div>
      </Field>
    </details>

    <Field
      v-slot="field"
      :of="form"
      :path="['authorized']"
    >
      <div class="space-y-2">
        <div class="flex items-center gap-2">
          <UCheckbox
            :model-value="field.input"
            :disabled="disabled"
            id="api-authorized"
            @update:model-value="field.input = $event === true"
          />

          <label
            class="text-sm font-medium"
            for="api-authorized"
          >
            I am authorized to send these requests to the selected targets.
          </label>
        </div>

        <p
          v-if="field.errors"
          class="text-error text-xs"
          role="alert"
        >
          {{ field.errors.join(' ') }}
        </p>
      </div>
    </Field>

    <div class="flex flex-wrap gap-3">
      <UButton
        :disabled="disabled || !hasRequest"
        type="submit"
        label="Send active request"
        @click="interactive = true"
      />

      <UButton
        :disabled="disabled"
        type="submit"
        variant="outline"
        color="neutral"
        label="Run selected scope"
        @click="interactive = false"
      />
    </div>
  </Form>
</template>
