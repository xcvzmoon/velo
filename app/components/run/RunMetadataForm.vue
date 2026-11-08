<script setup lang="ts">
  import { Field as FormischField, Form as FormischForm, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const schema = v.object({
    git_sha: v.pipe(
      v.string(),
      v.regex(/^(?:[a-fA-F0-9]{7,64})?$/, 'Use a 7–64 digit hexadecimal SHA.'),
    ),
    release: v.pipe(v.string(), v.maxBytes(120)),
    tags: v.pipe(
      v.string(),
      v.transform((value) =>
        value
          .split(',')
          .map((tag) => tag.trim())
          .filter(Boolean),
      ),
      v.array(v.pipe(v.string(), v.maxBytes(64))),
      v.maxLength(20),
    ),
    notes: v.pipe(v.string(), v.maxBytes(4000)),
  });
  const fields = [
    {
      name: 'git_sha',
      label: 'Git SHA',
    },
    {
      name: 'release',
      label: 'Release label',
    },
    {
      name: 'tags',
      label: 'Tags (comma separated)',
    },
    {
      name: 'notes',
      label: 'Notes',
    },
  ] as const;
  const props = defineProps<{ metadata: RunMetadata; disabled: boolean }>();
  const emit = defineEmits<{ save: [metadata: RunMetadata]; dirty: [] }>();
  const formId = useId();
  const form = useForm({
    schema,
    initialInput: {
      ...props.metadata,
      tags: props.metadata.tags.join(', '),
    },
  });
  watch(
    () => getInput(form),
    () => emit('dirty'),
    { deep: true },
  );
</script>

<template>
  <FormischForm
    :of="form"
    class="border-default space-y-3 rounded-md border p-4"
    @submit="emit('save', $event)"
  >
    <h4 class="text-sm font-medium">Release context</h4>

    <p class="text-muted text-sm">
      These fields are exported. Include only information safe to share.
    </p>

    <FormischField
      v-for="{ name, label } in fields"
      v-slot="field"
      :key="name"
      :of="form"
      :path="[name]"
    >
      <div class="space-y-2">
        <div class="grid min-w-0 gap-2">
          <label
            :for="`${formId}-${name}`"
            class="text-sm font-medium"
          >
            {{ label }}
          </label>

          <UTextarea
            v-if="name === 'notes'"
            v-model="field.input"
            v-bind="field.props"
            :id="`${formId}-${name}`"
            :disabled="disabled"
          />

          <UInput
            v-else
            v-model="field.input"
            v-bind="field.props"
            :id="`${formId}-${name}`"
            :disabled="disabled"
            autocomplete="off"
          />
        </div>

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
      type="submit"
      variant="outline"
      color="neutral"
      label="Save release context"
    />
  </FormischForm>
</template>
