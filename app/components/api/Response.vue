<script setup lang="ts">
  import type { Assertion, PreviewResponse } from '~/types/contracts';
  import { Field, Form, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    response: PreviewResponse;
    assertions?: Assertion[];
  }>();
  const form = useForm({
    schema: v.object({
      search: v.string(),
      format: v.picklist(['json', 'raw']),
    }),
    initialInput: {
      search: '',
      format: 'json',
    },
  });
  const prefix = useId();
  const error = shallowRef<string | null>(null);
  const display = computed<string>(() => {
    if (getInput(form).format === 'json') {
      try {
        return JSON.stringify(JSON.parse(props.response.body), null, 2) ?? props.response.body;
      } catch {
        return props.response.body;
      }
    }
    return props.response.body;
  });
  const matches = computed<number>(() => {
    const search = (getInput(form).search ?? '').toLowerCase();
    if (!search) {
      return 0;
    }
    return display.value.toLowerCase().split(search).length - 1;
  });
  async function copy(): Promise<void> {
    error.value = null;
    try {
      await navigator.clipboard.writeText(display.value);
    } catch {
      error.value = 'COPY_FAILED: The response could not be copied.';
    }
  }
  function download(): void {
    const url = URL.createObjectURL(
      new Blob([display.value], { type: 'text/plain;charset=utf-8' }),
    );
    try {
      const link = document.createElement('a');
      link.href = url;
      link.download = 'velo-response.txt';
      link.click();
    } finally {
      URL.revokeObjectURL(url);
    }
  }
</script>

<template>
  <section
    class="space-y-4 rounded-lg border p-5"
    aria-label="HTTP response"
  >
    <dl class="flex flex-wrap gap-5 text-sm">
      <div>
        <dt class="text-muted">Status</dt>
        <dd>{{ response.status }}</dd>
      </div>

      <div>
        <dt class="text-muted">Total elapsed</dt>
        <dd>{{ response.elapsed_ms.toFixed(2) }} ms</dd>
      </div>

      <div>
        <dt class="text-muted">Received</dt>
        <dd>{{ response.received_bytes }} bytes{{ response.truncated ? ' (partial)' : '' }}</dd>
      </div>
    </dl>

    <p
      v-if="!response.content_retained"
      class="text-muted text-sm"
    >
      Response content was not retained under the history policy. Send the request explicitly to
      inspect a new response.
    </p>

    <p class="text-muted text-xs">
      Elapsed time includes redirects and bounded body consumption. DNS, TLS, and connection timings
      are not measured. Body capture limit: {{ response.body_limit }} bytes.
    </p>

    <p
      v-if="response.truncated"
      class="text-warning text-sm"
      role="status"
    >
      Capture was truncated at the body or header limit. Received size may be a lower bound.
    </p>

    <p
      v-if="response.redacted"
      class="text-muted text-sm"
    >
      Sensitive response fields and known request values were redacted. Copy and export use this
      redacted content.
    </p>

    <p
      v-if="response.body_format !== 'text'"
      class="text-muted text-sm"
    >
      Content is {{ response.body_format.replaceAll('_', ' ') }}. The body cannot be displayed as
      supported text.
    </p>

    <Form
      :of="form"
      class="flex flex-wrap items-end gap-3"
      @submit="() => {}"
    >
      <Field
        v-slot="field"
        :of="form"
        :path="['format']"
      >
        <div class="space-y-1">
          <label
            :for="`${prefix}-format`"
            class="text-sm font-medium"
          >
            Body format
          </label>

          <USelect
            v-model="field.input"
            :id="`${prefix}-format`"
            :items="[
              { label: 'Formatted JSON', value: 'json' },
              { label: 'Raw text', value: 'raw' },
            ]"
            class="w-full"
          />
        </div>
      </Field>

      <Field
        v-slot="field"
        :of="form"
        :path="['search']"
      >
        <div class="space-y-1">
          <label
            :for="`${prefix}-search`"
            class="text-sm font-medium"
          >
            Find in body
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="`${prefix}-search`"
            type="search"
          />
        </div>
      </Field>

      <p
        class="text-muted text-sm"
        aria-live="polite"
      >
        {{ matches }} matches
      </p>

      <UButton
        :disabled="!response.content_retained"
        type="button"
        variant="outline"
        color="neutral"
        label="Copy redacted body"
        @click="copy"
      />

      <UButton
        :disabled="!response.content_retained"
        type="button"
        variant="outline"
        color="neutral"
        label="Export redacted body"
        @click="download"
      />
    </Form>

    <p
      v-if="error"
      role="alert"
      class="text-error text-sm"
    >
      {{ error }}
    </p>

    <pre
      class="bg-muted max-h-96 overflow-auto rounded-md p-4 font-mono text-xs break-all whitespace-pre-wrap"
      >{{ display }}</pre>

    <details>
      <summary class="cursor-pointer text-sm">Response headers and cookies</summary>

      <dl class="mt-3 space-y-2 font-mono text-xs">
        <div
          v-for="(header, index) in response.headers"
          :key="index"
          class="grid grid-cols-[minmax(6rem,1fr)_3fr] gap-3 break-all"
        >
          <dt>{{ header.name }}</dt>
          <dd>{{ header.value }}</dd>
        </div>
      </dl>
    </details>

    <div
      v-if="response.checks.length"
      class="space-y-2"
    >
      <h3 class="text-sm font-medium">Functional assertions</h3>

      <p
        v-for="(check, index) in response.checks"
        :key="index"
        class="font-mono text-xs"
      >
        {{ assertions?.[index]?.id ?? `Check ${index + 1}` }}: {{ check.replaceAll('_', ' ') }}
      </p>
    </div>
  </section>
</template>
