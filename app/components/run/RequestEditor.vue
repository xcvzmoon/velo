<script setup lang="ts">
  import type { RunConfig } from '~/types/contracts';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    Form as FormischForm,
    getInput,
    insert,
    remove,
    setInput,
    useForm,
  } from '@formisch/vue';
  import * as v from 'valibot';
  import { assertionInput } from '~/schemas/assertions';
  import { requestEditorSchema } from '~/schemas/run';

  const numericFields = [
    {
      name: 'timeout_ms',
      label: 'Request timeout (ms)',
      min: 1,
    },
    {
      name: 'pacing_ms',
      label: 'Iteration delay (ms)',
      min: 0,
    },
    {
      name: 'grace_ms',
      label: 'Drain grace (ms)',
      min: 0,
    },
    {
      name: 'max_vus',
      label: 'Worker budget',
      min: 1,
    },
    {
      name: 'max_request_bytes',
      label: 'Request-data budget (bytes)',
      min: 1,
    },
  ] as const;

  const pairFields = [
    {
      name: 'headers',
      label: 'Headers',
      singular: 'header',
    },
    {
      name: 'query',
      label: 'Query parameters',
      singular: 'parameter',
    },
  ] as const;

  const props = defineProps<{
    disabled: boolean;
    loaded?: RunConfig | null;
    stepMode?: boolean;
    scenarioMode?: boolean;
    apiMode?: boolean;
  }>();
  const emit = defineEmits<{
    prepare: [config: RunConfig];
    change: [config: RunConfig];
    valid: [value: boolean];
    dirty: [value: boolean];
  }>();

  const form = useForm({
    schema: requestEditorSchema,
    initialInput: {
      url: 'http://127.0.0.1:8080/',
      method: 'GET',
      authKind: 'none',
      authToken: '',
      authUsername: '',
      authPassword: '',
      apiKeyName: '',
      apiKeyValue: '',
      apiKeyPlacement: 'header',
      protocol: 'auto',
      max_redirects: 0,
      formPairs: [],
      multipartParts: [],
      thresholds: [],
      warmup_ms: 0,
      abort_policies: [],
      assertions: [],
      inspection: {
        body_bytes: 16_384,
        sample_count: 16,
        sample_bytes: 8_192,
      },
      headers: [],
      query: [],
      bodyKind: 'none',
      bodyValue: '',
      workloadKind: 'constant',
      rate: 10,
      capacity: 10,
      vus: 1,
      start_vus: 0,
      stages: [
        {
          duration_ms: 10_000,
          target_vus: 10,
        },
      ],
      duration_ms: 10_000,
      timeout_ms: 30_000,
      pacing_ms: 0,
      grace_ms: 5_000,
      max_vus: 256,
      max_request_bytes: 8_388_608,
    },
  });

  watch(
    () => props.loaded,
    (config) => {
      if (!config) {
        return;
      }
      const { workload, body, auth, budget, ...request } = config;
      setInput(form, {
        input: {
          ...request,
          warmup_ms: props.apiMode ? 0 : config.warmup_ms,
          thresholds: props.apiMode ? [] : config.thresholds,
          pacing_ms: props.apiMode ? 0 : config.pacing_ms,
          grace_ms: props.apiMode ? 5000 : config.grace_ms,
          abort_policies: props.apiMode ? [] : config.abort_policies,
          assertions: config.assertions.map((assertion) => assertionInput(assertion)),
          authKind: auth.kind,
          authToken: auth.kind === 'bearer' ? auth.token : '',
          authUsername: auth.kind === 'basic' ? auth.username : '',
          authPassword: auth.kind === 'basic' ? auth.password : '',
          apiKeyName: auth.kind === 'api_key' ? auth.name : '',
          apiKeyValue: auth.kind === 'api_key' ? auth.value : '',
          apiKeyPlacement: auth.kind === 'api_key' ? auth.placement : 'header',
          bodyKind: body.kind,
          bodyValue: body.kind === 'text' || body.kind === 'json' ? body.value : '',
          formPairs: body.kind === 'form' ? body.value : [],
          multipartParts:
            body.kind === 'multipart'
              ? body.value.map((part) => ({
                  kind: part.kind,
                  name: part.name,
                  value: part.kind === 'text' ? part.value : '',
                  path: part.kind === 'file' ? part.path : '',
                  filename: part.kind === 'file' ? part.filename : '',
                  content_type: part.kind === 'file' ? part.content_type : '',
                }))
              : [],
          workloadKind: props.apiMode ? 'constant' : workload.kind,
          vus: !props.apiMode && workload.kind === 'constant' ? workload.vus : 1,
          duration_ms: props.apiMode
            ? 1000
            : workload.kind === 'ramping'
              ? 10000
              : workload.duration_ms,
          start_vus: workload.kind === 'ramping' ? workload.start_vus : 0,
          stages: workload.kind === 'ramping' ? workload.stages : [],
          rate: workload.kind === 'arrival' ? workload.rate : 10,
          capacity: workload.kind === 'arrival' ? workload.capacity : 10,
          max_vus: props.apiMode ? 256 : budget.max_vus,
          max_request_bytes: props.apiMode ? 8388608 : budget.max_request_bytes,
        },
      });
    },
    { immediate: true },
  );

  watch(
    () => getInput(form),
    (input) => {
      const result = v.safeParse(requestEditorSchema, input);
      emit('valid', result.success);
      emit(
        'dirty',
        !result.success ||
          (Boolean(props.loaded) &&
            result.success &&
            JSON.stringify(result.output) !== JSON.stringify(props.loaded)),
      );
      if (result.success) {
        emit('change', result.output);
      }
    },
    {
      deep: true,
      immediate: true,
    },
  );

  function prepare(config: RunConfig): void {
    emit('prepare', config);
  }
</script>

<template>
  <ShellPanel
    :variant="apiMode || stepMode ? 'card' : 'section'"
    aria-labelledby="request-editor-heading"
  >
    <template #header>
      <h2
        :class="apiMode || stepMode ? 'text-base' : 'text-lg tracking-tight'"
        id="request-editor-heading"
        class="text-highlighted font-semibold"
      >
        {{ apiMode ? 'API request' : stepMode ? 'Step request' : 'Request configuration' }}
      </h2>
    </template>

    <UAlert
      v-if="!stepMode && !apiMode"
      :ui="{ title: 'text-highlighted', description: 'text-default leading-relaxed opacity-100' }"
      title="Execution model"
      icon="i-hugeicons-information-circle"
      color="info"
      variant="soft"
    >
      <template #description>
        <template v-if="scenarioMode">
          Multi-step scenarios are enabled. Workload, transport, timeout, inspection limits and
          budgets below apply to all steps. Edit each step's HTTP content in Scenarios. The
          standalone request supplies defaults for new steps.
        </template>

        <template v-else>
          Choose closed-loop users or a scheduled arrival rate. Closed-loop users wait for their
          response before sending another request. Automatic retries are disabled. Configured
          redirects stay on the target origin.
        </template>
      </template>
    </UAlert>

    <FormischForm
      :of="form"
      class="mt-6 space-y-6"
      @submit="prepare"
    >
      <fieldset
        :disabled="disabled || form.isSubmitting"
        class="space-y-6 disabled:opacity-60"
      >
        <legend class="sr-only">Request and workload settings</legend>

        <div class="grid gap-4 sm:grid-cols-[8rem_1fr]">
          <FormischField
            v-slot="field"
            :of="form"
            :path="['method']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  class="text-sm font-medium"
                  for="request-method"
                >
                  Method
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :aria-invalid="Boolean(field.errors)"
                  :items="[
                    { label: 'GET', value: 'GET' },
                    { label: 'POST', value: 'POST' },
                    { label: 'PUT', value: 'PUT' },
                    { label: 'PATCH', value: 'PATCH' },
                    { label: 'DELETE', value: 'DELETE' },
                    { label: 'HEAD', value: 'HEAD' },
                    { label: 'OPTIONS', value: 'OPTIONS' },
                  ]"
                  id="request-method"
                  class="w-full"
                />
              </div>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['url']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  class="text-sm font-medium"
                  for="request-url"
                >
                  Target URL
                </label>

                <UInput
                  v-model="field.input"
                  v-bind="field.props"
                  :aria-invalid="Boolean(field.errors)"
                  id="request-url"
                  aria-describedby="request-url-error"
                  type="url"
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>

              <p
                v-if="field.errors"
                id="request-url-error"
                class="text-error text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>
        </div>

        <FormischFieldArray
          v-for="pairGroup in pairFields"
          v-slot="fieldArray"
          :key="pairGroup.name"
          :of="form"
          :path="[pairGroup.name]"
        >
          <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
              <h3 class="text-sm font-medium">{{ pairGroup.label }}</h3>

              <UButton
                :disabled="disabled"
                :label="`Add ${pairGroup.singular}`"
                type="button"
                variant="outline"
                color="neutral"
                @click="
                  insert(form, { path: [pairGroup.name], initialInput: { name: '', value: '' } })
                "
              />
            </div>

            <p
              v-if="!fieldArray.items.length"
              class="text-muted text-xs"
            >
              No additional {{ pairGroup.label.toLowerCase() }}.
            </p>

            <div
              v-for="(item, index) in fieldArray.items"
              :key="item"
              class="grid items-start gap-2 sm:grid-cols-[1fr_1fr_auto]"
            >
              <FormischField
                v-slot="field"
                :of="form"
                :path="[pairGroup.name, index, 'name']"
              >
                <div class="space-y-1">
                  <div class="grid min-w-0 gap-2">
                    <label
                      :for="`${pairGroup.name}-${item}-name`"
                      class="sr-only text-sm font-medium"
                    >
                      {{ pairGroup.singular }} {{ index + 1 }} name
                    </label>

                    <UInput
                      v-model="field.input"
                      v-bind="field.props"
                      :id="`${pairGroup.name}-${item}-name`"
                      :aria-invalid="Boolean(field.errors)"
                      :aria-describedby="`${pairGroup.name}-${item}-name-error`"
                      placeholder="Name"
                      autocomplete="off"
                      spellcheck="false"
                    />
                  </div>

                  <p
                    v-if="field.errors"
                    :id="`${pairGroup.name}-${item}-name-error`"
                    class="text-error text-xs"
                    role="alert"
                  >
                    {{ field.errors[0] }}
                  </p>
                </div>
              </FormischField>

              <FormischField
                v-slot="field"
                :of="form"
                :path="[pairGroup.name, index, 'value']"
              >
                <div class="space-y-1">
                  <div class="grid min-w-0 gap-2">
                    <label
                      :for="`${pairGroup.name}-${item}-value`"
                      class="sr-only text-sm font-medium"
                    >
                      {{ pairGroup.singular }} {{ index + 1 }} value
                    </label>

                    <UInput
                      v-model="field.input"
                      v-bind="field.props"
                      :id="`${pairGroup.name}-${item}-value`"
                      :aria-invalid="Boolean(field.errors)"
                      :aria-describedby="`${pairGroup.name}-${item}-value-error`"
                      placeholder="Value"
                      autocomplete="off"
                      spellcheck="false"
                    />
                  </div>

                  <p
                    v-if="field.errors"
                    :id="`${pairGroup.name}-${item}-value-error`"
                    class="text-error text-xs"
                    role="alert"
                  >
                    {{ field.errors[0] }}
                  </p>
                </div>
              </FormischField>

              <UButton
                :aria-label="`Remove ${pairGroup.singular} ${index + 1}`"
                type="button"
                variant="ghost"
                color="neutral"
                label="Remove"
                @click="remove(form, { path: [pairGroup.name], at: index })"
              />
            </div>
          </div>
        </FormischFieldArray>

        <RunHttpOptionsEditor
          :form="form"
          :disabled="disabled"
          :hide-transport="stepMode"
        />

        <RunBodyEditor
          :form="form"
          :disabled="disabled"
        />

        <div
          v-if="!stepMode"
          class="border-default space-y-4 border-t pt-6"
        >
          <h3
            v-if="!apiMode"
            class="text-highlighted text-xs font-semibold tracking-wide uppercase"
          >
            Workload and local resource budgets
          </h3>

          <RunWorkloadEditor
            v-if="!apiMode"
            :form="form"
            :disabled="disabled"
          />

          <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            <FormischField
              v-for="option in apiMode ? numericFields.slice(0, 1) : numericFields"
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
                    :min="option.min"
                    :aria-invalid="Boolean(field.errors)"
                    :aria-describedby="`request-${option.name}-error`"
                    type="number"
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
            v-if="!apiMode"
            class="text-muted text-xs leading-relaxed"
          >
            Budgets bound owned workers and retained request data, including VU clones. They do not
            limit total process memory or certify this machine's load capacity. Stop closes new
            requests, drains for the configured grace, then cancels remaining requests.
          </p>
        </div>

        <RunAssertionEditor
          :hide-limits="stepMode"
          :form="form"
          :disabled="disabled"
        />

        <RunPolicyEditor
          v-if="!stepMode && !apiMode"
          :form="form"
          :disabled="disabled"
        />

        <RunThresholdEditor
          v-if="!stepMode && !apiMode"
          :form="form"
          :disabled="disabled"
        />

        <UButton
          v-if="!stepMode && !apiMode"
          :disabled="disabled || form.isSubmitting"
          type="submit"
          label="Review run"
        />
      </fieldset>
    </FormischForm>
  </ShellPanel>
</template>
