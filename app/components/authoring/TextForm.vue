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
        <Label :for="field.props.name">{{ label }}</Label>

        <Textarea
          v-model="field.input"
          v-bind="field.props"
          :id="field.props.name"
          :disabled="disabled"
          class="min-h-64 font-mono text-xs"
          spellcheck="false"
          autocomplete="off"
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

    <Button
      :disabled="disabled"
      type="submit"
    >
      {{ action }}
    </Button>
  </FormischForm>
</template>
