<script setup lang="ts">
  import type { DataDefinition, DataField } from '~/types/contracts';
  import { Field, Form, getInput, reset, setInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const defaultFields: DataField[] = [
    {
      name: 'firstName',
      generator: { kind: 'first_name' },
    },
    {
      name: 'lastName',
      generator: { kind: 'last_name' },
    },
    {
      name: 'email',
      generator: {
        kind: 'email',
        first: 'firstName',
        last: 'lastName',
        provider: 'example.test',
      },
    },
  ];
  const schema = v.object({
    kind: v.picklist(['faker', 'csv', 'json']),
    locale: v.picklist(['en', 'de', 'fr']),
    count: v.pipe(
      v.string(),
      v.transform(Number),
      v.number(),
      v.integer(),
      v.minValue(1),
      v.maxValue(10000),
    ),
    seed: v.pipe(
      v.string(),
      v.transform(Number),
      v.number(),
      v.integer(),
      v.minValue(0),
      v.maxValue(4294967295),
    ),
    lifetime: v.picklist(['run', 'user', 'iteration']),
    allocation: v.picklist(['sequential', 'random', 'unique']),
    exhaustion: v.picklist(['stop', 'repeat', 'fail']),
    fields: v.pipe(v.string(), v.maxLength(65536)),
    mapping: v.pipe(v.string(), v.maxLength(65536)),
  });
  const selectFields = [
    {
      name: 'kind',
      label: 'Source',
      choices: ['faker', 'csv', 'json'],
    },
    {
      name: 'locale',
      label: 'Faker locale',
      choices: ['en', 'de', 'fr'],
    },
    {
      name: 'lifetime',
      label: 'Reuse values for',
      choices: ['run', 'user', 'iteration'],
    },
    {
      name: 'allocation',
      label: 'Record allocation',
      choices: ['sequential', 'random', 'unique'],
    },
    {
      name: 'exhaustion',
      label: 'When records run out',
      choices: ['stop', 'repeat', 'fail'],
    },
  ] as const;
  const props = defineProps<{
    disabled: boolean;
    definition: DataDefinition | null;
  }>();
  const emit = defineEmits<{
    apply: [definition: DataDefinition | null];
    dirty: [value: boolean];
  }>();
  const form = useForm({
    schema,
    initialInput: {
      kind: 'faker',
      locale: 'en',
      count: '100',
      seed: '42',
      lifetime: 'iteration',
      allocation: 'unique',
      exhaustion: 'stop',
      fields: JSON.stringify(defaultFields, null, 2),
      mapping: '{"id":"id"}',
    },
  });
  const prefix = useId();
  const error = shallowRef<string | null>(null);
  function apply(output: v.InferOutput<typeof schema>): void {
    try {
      const source =
        output.kind === 'faker'
          ? {
              kind: 'faker',
              version: '10.6.0',
              locale: output.locale,
              count: output.count,
              fields: JSON.parse(output.fields),
            }
          : {
              kind: output.kind,
              mapping: JSON.parse(output.mapping),
            };
      emit(
        'apply',
        parseDataDefinition(
          JSON.stringify({
            lifetime: output.lifetime,
            allocation: output.allocation,
            exhaustion: output.exhaustion,
            seed: output.seed,
            source,
          }),
        ),
      );
      error.value = null;
    } catch (error_) {
      error.value = error_ instanceof Error ? error_.message : 'INVALID_DATASET: Check the recipe.';
    }
  }
  function addField(field: DataField): void {
    try {
      const parsed = parseDataDefinition(
        JSON.stringify({
          lifetime: 'iteration',
          allocation: 'unique',
          exhaustion: 'stop',
          seed: 0,
          source: {
            kind: 'faker',
            version: '10.6.0',
            locale: 'en',
            count: 1,
            fields: JSON.parse(getInput(form).fields ?? '[]'),
          },
        }),
      );
      if (parsed.source.kind === 'faker') {
        setInput(form, {
          path: ['fields'],
          input: JSON.stringify([...parsed.source.fields, field], null, 2),
        });
      }
    } catch {
      error.value = 'INVALID_DATASET: Fix the existing field JSON before adding a field.';
    }
  }
  watch(
    () => getInput(form),
    () => emit('dirty', form.isDirty),
    { deep: true },
  );
  watch(
    () => props.definition,
    (definition) => {
      if (!definition) {
        return;
      }
      const source = definition.source;
      reset(form, {
        initialInput: {
          kind: source.kind,
          locale:
            source.kind === 'faker' ? v.parse(v.picklist(['en', 'de', 'fr']), source.locale) : 'en',
          count: source.kind === 'faker' ? String(source.count) : '100',
          seed: String(definition.seed),
          lifetime: definition.lifetime,
          allocation: definition.allocation,
          exhaustion: definition.exhaustion,
          fields: JSON.stringify(source.kind === 'faker' ? source.fields : defaultFields, null, 2),
          mapping: JSON.stringify(source.kind === 'faker' ? { id: 'id' } : source.mapping, null, 2),
        },
      });
    },
    { immediate: true },
  );
</script>

<template>
  <ShellPanel
    variant="section"
    aria-label="Test data"
  >
    <template #header>
      <h2 class="text-highlighted text-xl font-semibold tracking-tight">Test data</h2>
    </template>

    <p class="text-default max-w-3xl text-base leading-relaxed">
      Prepare up to
      <strong class="text-highlighted font-semibold">10,000 records</strong>
      before load. Bind fields using
      <code v-text="'{{email}}'" />
      .
    </p>

    <UAlert
      :ui="{ title: 'text-highlighted', description: 'text-default leading-relaxed opacity-100' }"
      title="Generated data does not create accounts"
      description="Login identities must already exist on the target. Unique allocation reserves each record once, but generated values may contain duplicates."
      icon="i-hugeicons-alert-circle"
      color="warning"
      variant="soft"
    />

    <Form
      :of="form"
      class="space-y-4"
      @submit="apply"
    >
      <div class="grid gap-4 sm:grid-cols-2">
        <Field
          v-for="setting in selectFields"
          v-slot="field"
          :key="setting.name"
          :of="form"
          :path="[setting.name]"
        >
          <div class="space-y-2">
            <div class="grid min-w-0 gap-2">
              <label
                :for="`${prefix}-${setting.name}`"
                class="text-sm font-medium"
              >
                {{ setting.label }}
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :id="`${prefix}-${setting.name}`"
                :items="[...setting.choices.map((choice) => ({ label: choice, value: choice }))]"
                class="w-full"
              />
            </div>
          </div>
        </Field>

        <Field
          v-for="name in ['seed', 'count'] as const"
          v-slot="field"
          :key="name"
          :of="form"
          :path="[name]"
        >
          <div class="space-y-2">
            <div class="grid min-w-0 gap-2">
              <label
                :for="`${prefix}-${name}`"
                class="text-sm font-medium"
              >
                {{ name === 'seed' ? 'Seed' : 'Faker record count' }}
              </label>

              <UInput
                v-model="field.input"
                v-bind="field.props"
                :id="`${prefix}-${name}`"
                :disabled="disabled"
                inputmode="numeric"
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
        </Field>
      </div>

      <Field
        v-for="name in ['fields', 'mapping'] as const"
        v-slot="field"
        :key="name"
        :of="form"
        :path="[name]"
      >
        <div
          v-if="(getInput(form).kind === 'faker') === (name === 'fields')"
          class="space-y-2"
        >
          <div class="grid min-w-0 gap-2">
            <label
              :for="`${prefix}-${name}`"
              class="text-sm font-medium"
            >
              {{
                name === 'fields'
                  ? 'Ordered field definitions (JSON)'
                  : 'Variable-to-source-column mapping (JSON)'
              }}
            </label>

            <UTextarea
              v-model="field.input"
              v-bind="field.props"
              :id="`${prefix}-${name}`"
              :disabled="disabled"
              :ui="{ base: 'font-mono' }"
              spellcheck="false"
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
      </Field>

      <div class="flex gap-3">
        <UButton
          :disabled="disabled"
          type="submit"
          label="Apply data recipe"
        />

        <UButton
          :disabled="disabled || !definition"
          type="button"
          variant="outline"
          color="neutral"
          label="Remove dataset"
          @click="emit('apply', null)"
        />
      </div>
    </Form>

    <AuthoringDataField
      v-if="getInput(form).kind === 'faker'"
      :disabled="disabled"
      @add="addField"
    />

    <p
      v-if="error"
      class="text-error text-sm"
      role="alert"
    >
      {{ error }}
    </p>

    <p class="text-muted text-xs">
      Faker 10.6.0 is pinned. Email and copy fields reference earlier fields in the same record.
      Literal fields make invalid and boundary cases explicit. Random allocation uses a seeded
      shuffle without replacement per cycle. Concurrent admission determines which worker receives
      each index; network completion order is unspecified. Arrival workloads support run and
      iteration lifetimes.
    </p>
  </ShellPanel>
</template>
