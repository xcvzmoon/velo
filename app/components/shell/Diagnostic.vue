<script setup lang="ts">
  import { Field as FormischField, Form as FormischForm, useForm } from '@formisch/vue';
  import { diagnosticInputSchema } from '~/schemas/diagnostic';

  const { message, error, checkConnection } = useDiagnostic();
  const form = useForm<typeof diagnosticInputSchema>({
    schema: diagnosticInputSchema,
    initialInput: { label: 'Velo desktop' },
  });
</script>

<template>
  <section
    class="rounded-lg border p-6"
    aria-labelledby="diagnostic-heading"
  >
    <h2
      id="diagnostic-heading"
      class="text-base font-medium"
    >
      Native connection
    </h2>

    <p class="text-muted mt-2 max-w-lg text-sm leading-relaxed">
      Verify that the desktop app can communicate with the independent Rust engine. This check sends
      no HTTP requests.
    </p>

    <FormischForm
      :of="form"
      class="mt-6 space-y-4"
      @submit="checkConnection"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['label']"
      >
        <div class="max-w-sm space-y-2">
          <label
            class="text-sm font-medium"
            for="diagnostic-label"
          >
            Diagnostic label
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :disabled="form.isSubmitting"
            :aria-invalid="Boolean(field.errors)"
            id="diagnostic-label"
            aria-describedby="diagnostic-label-help diagnostic-label-error"
            autocomplete="off"
          />

          <p
            id="diagnostic-label-help"
            class="text-muted text-xs"
          >
            Use 1–64 characters. Do not include secrets.
          </p>

          <p
            v-if="field.errors"
            id="diagnostic-label-error"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors[0] }}
          </p>
        </div>
      </FormischField>

      <UButton
        :disabled="form.isSubmitting"
        :label="form.isSubmitting ? 'Checking…' : 'Check connection'"
        type="submit"
        variant="outline"
        color="neutral"
      />
    </FormischForm>

    <p
      :class="['mt-5 text-sm', error ? 'text-error' : 'text-muted']"
      role="status"
      aria-live="polite"
    >
      {{ message }}
    </p>
  </section>
</template>
