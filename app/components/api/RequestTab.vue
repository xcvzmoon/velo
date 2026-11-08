<script setup lang="ts">
  import type { CollectionEntry, RunConfig } from '~/types/contracts';
  import { Field, Form, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const props = defineProps<{
    entry: Extract<
      CollectionEntry,
      {
        kind: 'request';
      }
    >;
    disabled: boolean;
    collectionId: string;
    tabKey: string;
  }>();
  const emit = defineEmits<{
    change: [collectionId: string, id: string, config: RunConfig];
    metadata: [
      collectionId: string,
      id: string,
      name: string,
      description: string,
      notes: string,
      inheritAuth: boolean,
    ];
    valid: [key: string, value: boolean];
    dirty: [key: string, value: boolean];
  }>();
  const schema = v.object({
    name: v.pipe(v.string(), v.trim(), v.minLength(1), v.maxLength(80)),
    description: v.pipe(v.string(), v.maxLength(8192)),
    notes: v.pipe(v.string(), v.maxLength(16384)),
    inherit_auth: v.boolean(),
  });
  const form = useForm({
    schema,
    initialInput: {
      name: props.entry.name,
      description: props.entry.description,
      notes: props.entry.notes,
      inherit_auth: props.entry.inherit_auth,
    },
  });
  const inputId = useId();
  const loaded = shallowRef<RunConfig>(props.entry.definition.config);
  const fields = ['name', 'description', 'notes'] as const;
  const editorValid = shallowRef<boolean>(true);
  const metadataValid = computed<boolean>(() => v.safeParse(schema, getInput(form)).success);
  watch(
    [editorValid, metadataValid],
    ([editor, metadata]) => emit('valid', props.tabKey, editor && metadata),
    {
      immediate: true,
    },
  );
  watch(
    () => getInput(form),
    (input) => {
      emit('dirty', props.tabKey, true);
      const parsed = v.safeParse(schema, input);
      if (parsed.success) {
        emit(
          'metadata',
          props.collectionId,
          props.entry.id,
          parsed.output.name,
          parsed.output.description,
          parsed.output.notes,
          parsed.output.inherit_auth,
        );
      }
    },
    { deep: true },
  );
  function applyToken(reference: string): void {
    const config = {
      ...props.entry.definition.config,
      auth: {
        kind: 'bearer' as const,
        token: reference,
      },
    };
    loaded.value = config;
    emit('change', props.collectionId, props.entry.id, config);
  }
</script>

<template>
  <div class="space-y-5">
    <Form
      :of="form"
      class="space-y-3"
      @submit="() => {}"
    >
      <Field
        v-for="name in fields"
        v-slot="field"
        :key="name"
        :of="form"
        :path="[name]"
      >
        <div class="space-y-2">
          <label
            :for="`${inputId}-${name}`"
            class="text-sm font-medium capitalize"
          >
            {{ name }}
          </label>

          <UInput
            v-model="field.input"
            v-bind="field.props"
            :id="`${inputId}-${name}`"
            :disabled="disabled"
            :aria-invalid="Boolean(field.errors)"
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
      </Field>

      <Field
        v-slot="field"
        :of="form"
        :path="['inherit_auth']"
      >
        <div class="flex items-center gap-2">
          <UCheckbox
            :id="`${inputId}-inherit`"
            :model-value="field.input"
            :disabled="disabled"
            @update:model-value="field.input = $event === true"
          />

          <label
            :for="`${inputId}-inherit`"
            class="text-sm font-medium"
          >
            Inherit authentication when this request uses None
          </label>
        </div>
      </Field>
    </Form>

    <p class="text-muted text-xs">
      Scope precedence: collection defaults → outer folder → inner folder → request variables →
      environment → dataset. Request headers override inherited names. Use the resolved preview to
      inspect effective settings. Store sensitive values in Secrets and reference them with
      <code v-pre>{{secret:name}}</code>
      .
    </p>

    <RunRequestEditor
      :loaded="loaded"
      :disabled="disabled"
      api-mode
      @change="emit('change', collectionId, entry.id, $event)"
      @valid="editorValid = $event"
      @dirty="emit('dirty', tabKey, $event)"
    />

    <ApiOAuth
      :disabled="disabled"
      @token="applyToken"
    />
  </div>
</template>
