<script setup lang="ts">
  import type { DataField, DataGenerator } from '~/types/contracts';
  import { Field, Form, getInput, useForm } from '@formisch/vue';
  import * as v from 'valibot';

  const schema = v.object({
    name: v.pipe(v.string(), v.regex(/^[A-Za-z0-9_.-]{1,80}$/)),
    kind: v.picklist(['first_name', 'last_name', 'uuid', 'email', 'integer', 'literal', 'copy']),
    first: v.string(),
    last: v.string(),
    provider: v.string(),
    min: v.pipe(
      v.string(),
      v.transform(Number),
      v.number(),
      v.integer(),
      v.minValue(-2147483648),
      v.maxValue(2147483647),
    ),
    max: v.pipe(
      v.string(),
      v.transform(Number),
      v.number(),
      v.integer(),
      v.minValue(-2147483648),
      v.maxValue(2147483647),
    ),
    value: v.pipe(v.string(), v.maxLength(16384)),
    field: v.string(),
  });
  const props = defineProps<{
    disabled: boolean;
  }>();
  const emit = defineEmits<{
    add: [field: DataField];
  }>();
  const form = useForm({
    schema,
    initialInput: {
      name: 'id',
      kind: 'uuid',
      first: 'firstName',
      last: 'lastName',
      provider: 'example.test',
      min: '0',
      max: '100',
      value: '',
      field: '',
    },
  });
  const prefix = useId();
  const argumentFields = [
    {
      name: 'first',
      label: 'Earlier first-name field',
    },
    {
      name: 'last',
      label: 'Earlier last-name field',
    },
    {
      name: 'provider',
      label: 'Email provider',
    },
    {
      name: 'min',
      label: 'Minimum integer',
    },
    {
      name: 'max',
      label: 'Maximum integer',
    },
    {
      name: 'value',
      label: 'Literal value (may be invalid)',
    },
    {
      name: 'field',
      label: 'Earlier field to copy',
    },
  ] as const;
  function add(output: v.InferOutput<typeof schema>): void {
    let generator: DataGenerator;
    switch (output.kind) {
      case 'email':
        generator = {
          kind: 'email',
          first: output.first,
          last: output.last,
          provider: output.provider,
        };
        break;
      case 'integer':
        generator = {
          kind: 'integer',
          min: output.min,
          max: output.max,
        };
        break;
      case 'literal':
        generator = {
          kind: 'literal',
          value: output.value,
        };
        break;
      case 'copy':
        generator = {
          kind: 'copy',
          field: output.field,
        };
        break;
      default:
        generator = { kind: output.kind };
    }
    emit('add', {
      name: output.name,
      generator,
    });
  }
</script>

<template>
  <Form
    :of="form"
    class="space-y-3 rounded-md border p-4"
    @submit="add"
  >
    <p class="text-sm font-medium">Add a generated field</p>

    <Field
      v-slot="field"
      :of="form"
      :path="['name']"
    >
      <Label :for="`${prefix}-name`">Variable name</Label>
      <Input
        v-model="field.input"
        v-bind="field.props"
        :id="`${prefix}-name`"
        :disabled="props.disabled"
      />
      <p
        v-if="field.errors"
        class="text-destructive text-sm"
      >
        {{ field.errors.join(' ') }}
      </p>
    </Field>

    <Field
      v-slot="field"
      :of="form"
      :path="['kind']"
    >
      <Label :for="`${prefix}-kind`">Faker generator or explicit value</Label>
      <Select
        v-model="field.input"
        :disabled="props.disabled"
      >
        <SelectTrigger :id="`${prefix}-kind`"><SelectValue /></SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="kind in [
              'first_name',
              'last_name',
              'uuid',
              'email',
              'integer',
              'literal',
              'copy',
            ]"
            :key="kind"
            :value="kind"
          >
            {{ kind }}
          </SelectItem>
        </SelectContent>
      </Select>
    </Field>

    <Field
      v-for="argument in argumentFields"
      v-slot="field"
      :key="argument.name"
      :of="form"
      :path="[argument.name]"
    >
      <template
        v-if="
          (getInput(form).kind === 'email' &&
            ['first', 'last', 'provider'].includes(argument.name)) ||
          (getInput(form).kind === 'integer' && ['min', 'max'].includes(argument.name)) ||
          (getInput(form).kind === 'literal' && argument.name === 'value') ||
          (getInput(form).kind === 'copy' && argument.name === 'field')
        "
      >
        <Label :for="`${prefix}-${argument.name}`">{{ argument.label }}</Label>
        <Input
          v-model="field.input"
          v-bind="field.props"
          :id="`${prefix}-${argument.name}`"
          :disabled="props.disabled"
        />
        <p
          v-if="field.errors"
          class="text-destructive text-sm"
        >
          {{ field.errors.join(' ') }}
        </p>
      </template>
    </Field>

    <Button
      :disabled="props.disabled"
      type="submit"
      variant="outline"
    >
      Add field to recipe
    </Button>
  </Form>
</template>
