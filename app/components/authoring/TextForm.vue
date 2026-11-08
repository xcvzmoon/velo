<script setup lang="ts">
  import {
    Field as FormischField,
    Form as FormischForm,
    getInput,
    setInput,
    useForm,
  } from '@formisch/vue';
  import * as v from 'valibot';

  const schema = v.object({ text: v.pipe(v.string(), v.maxLength(8_388_608)) });
  const props = defineProps<{ source: string; label: string; action: string; disabled: boolean }>();
  const emit = defineEmits<{ apply: [source: string]; dirty: [value: boolean] }>();
  const inputId = useId();
  const form = useForm({
    schema,
    initialInput: { text: props.source },
  });

  watch(
    () => getInput(form).text,
    (text) => emit('dirty', text !== props.source),
  );

  watch(
    () => props.source,
    (text) => setInput(form, { input: { text } }),
  );
</script>

<template>
  <FormischForm
    :of="form"
    class="space-y-3"
    @submit="emit('apply', $event.text)"
  >
    <FormischField
      v-slot="field"
      :of="form"
      :path="['text']"
    >
      <div class="space-y-2">
        <label
          :for="inputId"
          class="text-sm font-medium"
        >
          {{ label }}
        </label>

        <UTextarea
          v-model="field.input"
          v-bind="field.props"
          :id="inputId"
          :disabled="disabled"
          :ui="{ base: 'min-h-64 font-mono text-xs md:text-xs' }"
          spellcheck="false"
          autocomplete="off"
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

    <UButton
      :disabled="disabled"
      :label="action"
      type="submit"
    />
  </FormischForm>
</template>
