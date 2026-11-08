<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import {
    Field as FormischField,
    FieldArray as FormischFieldArray,
    insert,
    remove,
  } from '@formisch/vue';

  const kinds = [
    {
      value: 'status',
      label: 'Allowed statuses',
    },
    {
      value: 'header_present',
      label: 'Header present',
    },
    {
      value: 'header_equals',
      label: 'Header equals',
    },
    {
      value: 'json_present',
      label: 'JSON field present',
    },
    {
      value: 'json_equals',
      label: 'JSON field equals',
    },
    {
      value: 'text_contains',
      label: 'Text contains',
    },
    {
      value: 'text_equals',
      label: 'Text equals',
    },
  ];
  const fields = [
    {
      name: 'id',
      label: 'Unique assertion ID',
      placeholder: 'application-ok',
    },
    {
      name: 'target',
      label: 'Statuses, header name or JSON pointer',
      placeholder: '200,204 or content-type or /data/ok',
    },
    {
      name: 'expected',
      label: 'Expected value (equality / text checks)',
      placeholder: 'true or application/json or literal text',
    },
  ] as const;
  const limits = [
    {
      name: 'body_bytes',
      label: 'Body inspection bytes per worker',
      min: 1,
      max: 65_536,
    },
    {
      name: 'sample_count',
      label: 'Retained samples per run',
      min: 0,
      max: 32,
    },
    {
      name: 'sample_bytes',
      label: 'Retained sample bytes per run',
      min: 0,
      max: 32_768,
    },
  ] as const;
  defineProps<{
    form: FormStore<typeof requestEditorSchema>;
    disabled: boolean;
    hideLimits?: boolean;
  }>();
  const editorId = useId();
</script>

<template>
  <div class="border-default space-y-4 border-t pt-6">
    <h3 class="text-sm font-medium">Response assertions and diagnostics</h3>

    <p class="text-muted text-xs leading-relaxed">
      Check the final response after redirects. JSON fields use RFC 6901 pointers, such as /data/ok;
      an empty pointer selects the document. JSON equality uses a JSON value, including quotes for
      strings. Text checks use exact UTF-8 literals. Each check has its own outcome; assertion
      failures leave the HTTP status classification unchanged.
    </p>

    <FormischFieldArray
      v-slot="rows"
      :of="form"
      :path="['assertions']"
    >
      <div class="space-y-4">
        <UButton
          :disabled="disabled || rows.items.length >= 32"
          type="button"
          variant="outline"
          color="neutral"
          label="Add assertion"
          @click="
            insert(form, {
              path: ['assertions'],
              initialInput: { id: '', kind: 'status', target: '200', expected: '' },
            })
          "
        />

        <p
          v-if="rows.errors"
          class="text-error text-xs"
          role="alert"
        >
          {{ rows.errors[0] }}
        </p>

        <p
          v-if="!rows.items.length"
          class="text-muted text-xs"
        >
          No response assertions configured.
        </p>

        <div
          v-for="(item, index) in rows.items"
          :key="item"
          class="border-default space-y-3 rounded-md border p-4"
        >
          <FormischField
            v-slot="field"
            :of="form"
            :path="['assertions', index, 'kind']"
          >
            <div class="space-y-2">
              <div class="grid min-w-0 gap-2">
                <label
                  :for="`${editorId}-${item}-kind`"
                  class="text-sm font-medium"
                >
                  Condition
                </label>

                <USelect
                  v-model="field.input"
                  :disabled="disabled"
                  :name="field.props.name"
                  :id="`${editorId}-${item}-kind`"
                  :items="[...kinds.map((kind) => ({ label: kind.label, value: kind.value }))]"
                  class="w-full"
                />
              </div>
            </div>
          </FormischField>

          <div class="grid gap-3 sm:grid-cols-3">
            <FormischField
              v-for="entry in fields"
              v-slot="field"
              :key="entry.name"
              :of="form"
              :path="['assertions', index, entry.name]"
            >
              <div class="space-y-2">
                <div class="grid min-w-0 gap-2">
                  <label
                    :for="`${editorId}-${item}-${entry.name}`"
                    class="text-sm font-medium"
                  >
                    {{ entry.label }}
                  </label>

                  <UInput
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`${editorId}-${item}-${entry.name}`"
                    :aria-invalid="Boolean(field.errors)"
                    :aria-describedby="`${editorId}-${item}-${entry.name}-error`"
                    :placeholder="entry.placeholder"
                  />
                </div>

                <p
                  v-if="field.errors"
                  :id="`${editorId}-${item}-${entry.name}-error`"
                  class="text-error text-xs"
                  role="alert"
                >
                  {{ field.errors[0] }}
                </p>
              </div>
            </FormischField>
          </div>

          <UButton
            :disabled="disabled"
            :aria-label="`Remove assertion ${index + 1}`"
            type="button"
            variant="ghost"
            color="neutral"
            label="Remove"
            @click="remove(form, { path: ['assertions'], at: index })"
          />
        </div>
      </div>
    </FormischFieldArray>

    <div
      v-if="!hideLimits"
      class="grid gap-3 sm:grid-cols-3"
    >
      <FormischField
        v-for="limit in limits"
        v-slot="field"
        :key="limit.name"
        :of="form"
        :path="['inspection', limit.name]"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${editorId}-${limit.name}`"
              class="text-sm font-medium"
            >
              {{ limit.label }}
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`${editorId}-${limit.name}`"
              :min="limit.min"
              :max="limit.max"
              :aria-invalid="Boolean(field.errors)"
              type="number"
              step="1"
            />
          </div>

          <p
            v-if="field.errors"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors[0] }}
          </p>
        </div>
      </FormischField>
    </div>

    <p class="text-muted text-xs leading-relaxed">
      Excess body bytes are drained and discarded. Body checks on truncated, encoded or invalid
      input are unavailable, never passed. Transport failures, timeouts and cancellations make all
      checks unavailable. Response latency includes body reading and bounded assertion processing.
      Samples retain only status, assertion index and cause; response bodies, headers and expected
      values are withheld. Set either sample limit to zero to disable samples. Samples follow run
      history retention and are deleted with the run.
    </p>
  </div>
</template>
