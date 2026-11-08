<script setup lang="ts">
  import { Field, Form, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    label: string;
    disabled: boolean;
  }>();
  const emit = defineEmits<{
    submit: [name: string];
  }>();
  const inputId = useId();
  const form = useForm({
    schema: v.object({
      name: v.pipe(v.string(), v.trim(), v.minLength(1, 'Enter a name.'), v.maxLength(80)),
    }),
    initialInput: { name: '' },
  });
</script>

<template>
  <Form
    :of="form"
    class="flex items-end gap-2"
    @submit="emit('submit', $event.name)"
  >
    <Field
      v-slot="field"
      :of="form"
      :path="['name']"
    >
      <div class="min-w-0 flex-1 space-y-1">
        <Label :for="inputId">{{ label }}</Label>

        <Input
          v-model="field.input"
          v-bind="field.props"
          :id="inputId"
          :disabled="disabled"
          :aria-invalid="Boolean(field.errors)"
          autocomplete="off"
        />

        <p
          v-if="field.errors"
          class="text-destructive text-xs"
          role="alert"
        >
          {{ field.errors.join(' ') }}
        </p>
      </div>
    </Field>

    <Button
      :disabled="disabled"
      type="submit"
    >
      Create
    </Button>
  </Form>
</template>
