<script setup lang="ts">
  import type { Extraction, RunConfig, ScenarioStep } from '~/types/contracts';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    Form as FormischForm,
    getInput,
    insert,
    remove,
    useForm,
  } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    step: ScenarioStep;
    base: RunConfig;
    disabled: boolean;
    reservedIds: string[];
  }>();
  const schema = v.object({
    id: v.pipe(
      v.string(),
      v.regex(/^[A-Za-z0-9_-]{1,64}$/, 'Use 1 to 64 letters, digits, underscores or hyphens.'),
      v.check((id) => !props.reservedIds.includes(id), 'Step names must be unique in this phase.'),
    ),

    think_ms: v.pipe(
      v.union([v.string(), v.number()]),
      v.toNumber(),
      v.integer(),
      v.minValue(0),
      v.maxValue(3_600_000),
    ),

    on_failure: v.picklist(['stop', 'continue']),

    extractions: v.pipe(
      v.array(
        v.object({
          name: v.pipe(v.string(), v.regex(/^[A-Za-z0-9_-]{1,64}$/)),

          kind: v.picklist(['json', 'header', 'cookie']),

          selector: v.pipe(v.string(), v.maxLength(1024)),

          required: v.boolean(),

          overwrite: v.boolean(),
        }),
      ),
      v.maxLength(32),
      v.check(
        (items) => new Set(items.map(({ name }) => name)).size === items.length,
        'Extraction names must be unique.',
      ),
    ),
  });

  const emit = defineEmits<{
    change: [step: ScenarioStep];
    dirty: [value: boolean];
    valid: [value: boolean];
  }>();
  const formId = useId();
  const initialRequest: RunConfig = {
    ...props.base,

    ...props.step.request,

    scenarios: undefined,

    data: undefined,
  };
  const form = useForm({
    schema,

    initialInput: {
      id: props.step.id,

      think_ms: props.step.think_ms,

      on_failure: props.step.on_failure,

      extractions: props.step.extractions.map(({ name, source, required, overwrite }) => ({
        name,

        kind: source.kind,

        selector: source.kind === 'json' ? source.pointer : source.name,

        required,

        overwrite,
      })),
    },
  });

  function apply(value: v.InferOutput<typeof schema>): void {
    const extractions: Extraction[] = value.extractions.map(
      ({ name, kind, selector, required, overwrite }) => ({
        name,

        source:
          kind === 'json'
            ? {
                kind,
                pointer: selector,
              }
            : {
                kind,
                name: selector,
              },

        required,

        overwrite,
      }),
    );
    emit('change', {
      ...props.step,

      id: value.id,

      think_ms: value.think_ms,

      on_failure: value.on_failure,

      extractions,
    });
    emit('dirty', false);
  }
  function applyRequest(config: RunConfig): void {
    const { url, method, headers, query, body, auth, assertions } = config;
    emit('change', {
      ...props.step,

      request: { url, method, headers, query, body, auth, assertions },
    });
  }
  watch(
    () => getInput(form),
    () => emit('dirty', true),
    { deep: true },
  );
</script>

<template>
  <div class="space-y-4">
    <FormischForm
      :of="form"
      class="space-y-4 rounded-md border p-4"
      @submit="apply"
    >
      <div class="grid gap-4 sm:grid-cols-2">
        <FormischField
          v-slot="field"
          :of="form"
          :path="['id']"
        >
          <div class="space-y-2">
            <label
              :for="`${formId}-id`"
              class="text-sm font-medium"
            >
              Step name / stable ID
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${formId}-id`"
              :disabled="disabled"
              autocomplete="off"
            />

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
          :path="['think_ms']"
        >
          <div class="space-y-2">
            <label
              :for="`${formId}-think`"
              class="text-sm font-medium"
            >
              Think time after step (ms)
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${formId}-think`"
              :disabled="disabled"
              type="number"
              min="0"
              max="3600000"
              step="1"
            />

            <p
              v-if="field.errors"
              class="text-error text-xs"
              role="alert"
            >
              {{ field.errors.join(' ') }}
            </p>
          </div>
        </FormischField>
      </div>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['on_failure']"
      >
        <div class="space-y-2">
          <label
            :for="`${formId}-failure`"
            class="text-sm font-medium"
          >
            On HTTP, assertion or required extraction failure
          </label>

          <USelect
            v-model="field.input"
            :disabled="disabled"
            :name="field.props.name"
            :id="`${formId}-failure`"
            :items="[
              { label: 'Stop this iteration', value: 'stop' },
              { label: 'Continue independent steps', value: 'continue' },
            ]"
            class="w-full"
          />
        </div>
      </FormischField>

      <FormischFieldArray
        v-slot="rows"
        :of="form"
        :path="['extractions']"
      >
        <div class="space-y-4">
          <h3 class="text-sm font-medium">Response extraction</h3>

          <p class="text-muted text-xs">
            Use JSON pointers such as /token, a response header name, or a Set-Cookie name.
            Reference a value later with
            <code v-text="'{{session:name}}'" />
            . Missing optional values remove stale values. Overwrites require explicit permission.
          </p>

          <div
            v-for="(item, index) in rows.items"
            :key="item"
            class="space-y-3 rounded-md border p-3"
          >
            <FormischField
              v-slot="field"
              :of="form"
              :path="['extractions', index, 'name']"
            >
              <div class="space-y-1">
                <label
                  :for="`${formId}-${item}-name`"
                  class="text-sm font-medium"
                >
                  Session variable name
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`${formId}-${item}-name`"
                  :disabled="disabled"
                  autocomplete="off"
                />

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
              :path="['extractions', index, 'kind']"
            >
              <div class="space-y-1">
                <label
                  :for="`${formId}-${item}-kind`"
                  class="text-sm font-medium"
                >
                  Source
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :id="`${formId}-${item}-kind`"
                  :items="[
                    { label: 'JSON field', value: 'json' },
                    { label: 'Response header', value: 'header' },
                    { label: 'Response cookie', value: 'cookie' },
                  ]"
                  class="w-full"
                />
              </div>
            </FormischField>

            <FormischField
              v-slot="field"
              :of="form"
              :path="['extractions', index, 'selector']"
            >
              <div class="space-y-1">
                <label
                  :for="`${formId}-${item}-selector`"
                  class="text-sm font-medium"
                >
                  JSON pointer, header or cookie name
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`${formId}-${item}-selector`"
                  :disabled="disabled"
                  autocomplete="off"
                />
              </div>
            </FormischField>

            <div class="flex flex-wrap gap-4">
              <FormischField
                v-slot="field"
                :of="form"
                :path="['extractions', index, 'required']"
              >
                <div class="space-y-1">
                  <label
                    :for="`${formId}-${item}-required`"
                    class="text-sm font-medium"
                  >
                    Required
                  </label>

                  <USelect
                    :model-value="field.input ? 'yes' : 'no'"
                    :disabled="disabled"
                    :id="`${formId}-${item}-required`"
                    :items="[
                      { label: 'Yes', value: 'yes' },
                      { label: 'No', value: 'no' },
                    ]"
                    class="w-full"
                    @update:model-value="field.input = $event === 'yes'"
                  />
                </div>
              </FormischField>

              <FormischField
                v-slot="field"
                :of="form"
                :path="['extractions', index, 'overwrite']"
              >
                <div class="space-y-1">
                  <label
                    :for="`${formId}-${item}-overwrite`"
                    class="text-sm font-medium"
                  >
                    Allow overwrite
                  </label>

                  <USelect
                    :model-value="field.input ? 'yes' : 'no'"
                    :disabled="disabled"
                    :id="`${formId}-${item}-overwrite`"
                    :items="[
                      { label: 'Yes', value: 'yes' },
                      { label: 'No', value: 'no' },
                    ]"
                    class="w-full"
                    @update:model-value="field.input = $event === 'yes'"
                  />
                </div>
              </FormischField>
            </div>

            <UButton
              :disabled="disabled"
              type="button"
              variant="ghost"
              color="neutral"
              label="Remove extraction"
              @click="remove(form, { path: ['extractions'], at: index })"
            />
          </div>

          <p
            v-if="rows.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ rows.errors.join(' ') }}
          </p>

          <UButton
            :disabled="disabled || rows.items.length >= 32"
            type="button"
            variant="outline"
            color="neutral"
            label="Add extraction"
            @click="
              insert(form, {
                path: ['extractions'],
                initialInput: {
                  name: '',
                  kind: 'json',
                  selector: '',
                  required: true,
                  overwrite: false,
                },
              })
            "
          />
        </div>
      </FormischFieldArray>

      <UButton
        :disabled="disabled"
        type="submit"
        label="Apply step settings"
      />
    </FormischForm>

    <RunRequestEditor
      :loaded="initialRequest"
      :disabled="disabled"
      step-mode
      @change="applyRequest"
      @valid="emit('valid', $event)"
    />

    <p class="text-muted text-xs">
      Transport, timeout, inspection limits, workload and budgets come from the main request
      configuration. Step requests execute sequentially within each iteration.
    </p>
  </div>
</template>
