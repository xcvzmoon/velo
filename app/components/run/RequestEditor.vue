<script setup lang="ts">
  import type { RunConfig } from '~/types/contracts';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    Form as FormischForm,
    insert,
    remove,
    useForm,
  } from '@formisch/vue';
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

  defineProps<{ disabled: boolean }>();
  const emit = defineEmits<{ prepare: [config: RunConfig] }>();

  const form = useForm({
    schema: requestEditorSchema,
    initialInput: {
      url: 'http://127.0.0.1:8080/',
      method: 'GET',
      headers: [],
      query: [],
      bodyKind: 'none',
      bodyValue: '',
      workloadKind: 'constant',
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

  function prepare(config: RunConfig): void {
    emit('prepare', config);
  }
</script>

<template>
  <section
    class="rounded-lg border p-6"
    aria-labelledby="request-editor-heading"
  >
    <h2
      id="request-editor-heading"
      class="text-base font-medium"
    >
      Request configuration
    </h2>

    <p class="text-muted-foreground mt-2 text-sm leading-relaxed">
      Repeat one request with constant or ramping virtual users. Each user waits for its response
      before sending another request. Redirects and automatic retries are disabled.
    </p>

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
              <Label for="request-method">Method</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
              >
                <SelectTrigger
                  :aria-invalid="Boolean(field.errors)"
                  id="request-method"
                  class="w-full"
                >
                  <SelectValue />
                </SelectTrigger>

                <SelectContent>
                  <SelectItem value="GET">GET</SelectItem>
                  <SelectItem value="POST">POST</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['url']"
          >
            <div class="space-y-2">
              <Label for="request-url">Target URL</Label>

              <Input
                v-model="field.input"
                v-bind="field.props"
                :aria-invalid="Boolean(field.errors)"
                id="request-url"
                aria-describedby="request-url-error"
                type="url"
                autocomplete="off"
                spellcheck="false"
              />

              <p
                v-if="field.errors"
                id="request-url-error"
                class="text-destructive text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>
        </div>

        <FormischFieldArray
          v-for="pairGroup in pairFields"
          :key="pairGroup.name"
          v-slot="fieldArray"
          :of="form"
          :path="[pairGroup.name]"
        >
          <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
              <h3 class="text-sm font-medium">{{ pairGroup.label }}</h3>

              <Button
                :disabled="disabled"
                type="button"
                size="sm"
                variant="outline"
                @click="
                  insert(form, { path: [pairGroup.name], initialInput: { name: '', value: '' } })
                "
              >
                Add {{ pairGroup.singular }}
              </Button>
            </div>

            <p
              v-if="!fieldArray.items.length"
              class="text-muted-foreground text-xs"
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
                  <Label
                    :for="`${pairGroup.name}-${item}-name`"
                    class="sr-only"
                  >
                    {{ pairGroup.singular }} {{ index + 1 }} name
                  </Label>

                  <Input
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`${pairGroup.name}-${item}-name`"
                    :aria-invalid="Boolean(field.errors)"
                    :aria-describedby="`${pairGroup.name}-${item}-name-error`"
                    placeholder="Name"
                    autocomplete="off"
                    spellcheck="false"
                  />

                  <p
                    v-if="field.errors"
                    :id="`${pairGroup.name}-${item}-name-error`"
                    class="text-destructive text-xs"
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
                  <Label
                    :for="`${pairGroup.name}-${item}-value`"
                    class="sr-only"
                  >
                    {{ pairGroup.singular }} {{ index + 1 }} value
                  </Label>

                  <Input
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`${pairGroup.name}-${item}-value`"
                    :aria-invalid="Boolean(field.errors)"
                    :aria-describedby="`${pairGroup.name}-${item}-value-error`"
                    placeholder="Value"
                    autocomplete="off"
                    spellcheck="false"
                  />

                  <p
                    v-if="field.errors"
                    :id="`${pairGroup.name}-${item}-value-error`"
                    class="text-destructive text-xs"
                    role="alert"
                  >
                    {{ field.errors[0] }}
                  </p>
                </div>
              </FormischField>

              <Button
                :aria-label="`Remove ${pairGroup.singular} ${index + 1}`"
                type="button"
                variant="ghost"
                @click="remove(form, { path: [pairGroup.name], at: index })"
              >
                Remove
              </Button>
            </div>
          </div>
        </FormischFieldArray>

        <div class="space-y-4">
          <FormischField
            v-slot="field"
            :of="form"
            :path="['bodyKind']"
          >
            <div class="space-y-2">
              <Label for="request-body-kind">Body format</Label>

              <Select
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
              >
                <SelectTrigger
                  :aria-invalid="Boolean(field.errors)"
                  id="request-body-kind"
                  aria-describedby="request-body-kind-error"
                  class="w-full sm:w-48"
                >
                  <SelectValue />
                </SelectTrigger>

                <SelectContent>
                  <SelectItem value="none">No body</SelectItem>
                  <SelectItem value="text">Plain text</SelectItem>
                  <SelectItem value="json">JSON</SelectItem>
                </SelectContent>
              </Select>

              <p
                v-if="field.errors"
                id="request-body-kind-error"
                class="text-destructive text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>

          <FormischField
            v-slot="field"
            :of="form"
            :path="['bodyValue']"
          >
            <div class="space-y-2">
              <Label for="request-body">Request body (POST only)</Label>

              <Textarea
                v-model="field.input"
                v-bind="field.props"
                :aria-invalid="Boolean(field.errors)"
                id="request-body"
                aria-describedby="request-body-help request-body-error"
                class="min-h-28 font-mono text-xs"
                autocomplete="off"
                spellcheck="false"
              />

              <p
                id="request-body-help"
                class="text-muted-foreground text-xs"
              >
                Select plain text or JSON to send this content. No body discards the content.
              </p>

              <p
                v-if="field.errors"
                id="request-body-error"
                class="text-destructive text-xs"
                role="alert"
              >
                {{ field.errors[0] }}
              </p>
            </div>
          </FormischField>
        </div>

        <div class="space-y-4 border-t pt-6">
          <h3 class="text-sm font-medium">Workload and local resource budgets</h3>

          <RunWorkloadEditor
            :form="form"
            :disabled="disabled"
          />

          <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            <FormischField
              v-for="option in numericFields"
              :key="option.name"
              v-slot="field"
              :of="form"
              :path="[option.name]"
            >
              <div class="space-y-2">
                <Label :for="`request-${option.name}`">{{ option.label }}</Label>

                <Input
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`request-${option.name}`"
                  :min="option.min"
                  :aria-invalid="Boolean(field.errors)"
                  :aria-describedby="`request-${option.name}-error`"
                  type="number"
                  step="1"
                />

                <p
                  v-if="field.errors"
                  :id="`request-${option.name}-error`"
                  class="text-destructive text-xs"
                  role="alert"
                >
                  {{ field.errors[0] }}
                </p>
              </div>
            </FormischField>
          </div>

          <p class="text-muted-foreground text-xs leading-relaxed">
            Budgets bound owned workers and retained request data, including VU clones. They do not
            limit total process memory or certify this machine's load capacity. Stop closes new
            requests, drains for the configured grace, then cancels remaining requests.
          </p>
        </div>

        <Button
          :disabled="disabled || form.isSubmitting"
          type="submit"
        >
          Review run
        </Button>
      </fieldset>
    </FormischForm>
  </section>
</template>
