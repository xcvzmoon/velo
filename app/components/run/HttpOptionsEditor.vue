<script setup lang="ts">
  import type { FormStore } from '@formisch/vue';
  import type { requestEditorSchema } from '~/schemas/run';
  import { Field as FormischField } from '@formisch/vue';

  const credentialFields = [
    {
      name: 'authToken',
      label: 'Bearer token',
      kind: 'bearer',
    },
    {
      name: 'authUsername',
      label: 'Username',
      kind: 'basic',
    },
    {
      name: 'authPassword',
      label: 'Password',
      kind: 'basic',
    },
  ] as const;

  defineProps<{
    form: FormStore<typeof requestEditorSchema>;
    disabled: boolean;
    hideTransport?: boolean;
  }>();
</script>

<template>
  <div class="space-y-4 border-t pt-6">
    <h3 class="text-sm font-medium">Authentication and transport</h3>

    <p class="text-muted-foreground text-xs leading-relaxed">
      Credentials are excluded from reports. Saving omits them unless you explicitly remember the
      request in your OS credential store. Use HTTPS to protect them in transit. Redirects stay on
      the original origin; other origins and HTTPS downgrades fail as transport errors. HTTP/2
      requires server support and uses prior knowledge over cleartext.
    </p>

    <FormischField
      v-slot="mode"
      :of="form"
      :path="['authKind']"
    >
      <div class="space-y-4">
        <div class="max-w-xs space-y-2">
          <Label for="request-auth">Authentication</Label>

          <Select
            v-model="mode.input"
            :disabled="disabled"
            :name="mode.props.name"
          >
            <SelectTrigger
              id="request-auth"
              :aria-invalid="Boolean(mode.errors)"
              aria-describedby="request-auth-error"
              class="w-full"
            >
              <SelectValue />
            </SelectTrigger>

            <SelectContent>
              <SelectItem value="none">None / custom header</SelectItem>
              <SelectItem value="bearer">Bearer token</SelectItem>
              <SelectItem value="basic">Basic</SelectItem>
            </SelectContent>
          </Select>

          <p
            v-if="mode.errors"
            id="request-auth-error"
            class="text-destructive text-xs"
            role="alert"
          >
            {{ mode.errors[0] }}
          </p>
        </div>

        <div
          v-if="mode.input !== 'none'"
          class="grid gap-4 sm:grid-cols-2"
        >
          <template
            v-for="option in credentialFields"
            :key="option.name"
          >
            <FormischField
              v-if="mode.input === option.kind"
              v-slot="field"
              :of="form"
              :path="[option.name]"
            >
              <div class="space-y-2">
                <Label :for="`request-${option.name}`">{{ option.label }}</Label>

                <Input
                  v-model="field.input"
                  v-bind="field.props"
                  :id="`request-${option.name}`"
                  :type="option.name === 'authUsername' ? 'text' : 'password'"
                  autocomplete="off"
                  spellcheck="false"
                />
              </div>
            </FormischField>
          </template>
        </div>
      </div>
    </FormischField>

    <div
      v-if="!hideTransport"
      class="grid gap-4 sm:grid-cols-2"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['protocol']"
      >
        <div class="space-y-2">
          <Label for="request-protocol">HTTP protocol</Label>

          <Select
            v-model="field.input"
            :disabled="disabled"
            :name="field.props.name"
          >
            <SelectTrigger
              id="request-protocol"
              class="w-full"
            >
              <SelectValue />
            </SelectTrigger>

            <SelectContent>
              <SelectItem value="auto">Negotiate HTTP/1.1 or HTTP/2</SelectItem>
              <SelectItem value="http1">Require HTTP/1.1</SelectItem>
              <SelectItem value="http2">Require HTTP/2</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['max_redirects']"
      >
        <div class="space-y-2">
          <Label for="request-redirects">Maximum redirects (0 disables)</Label>

          <Input
            v-model="field.input"
            v-bind="field.props"
            :aria-invalid="Boolean(field.errors)"
            id="request-redirects"
            aria-describedby="request-redirects-error"
            type="number"
            min="0"
            step="1"
          />

          <p
            v-if="field.errors"
            id="request-redirects-error"
            class="text-destructive text-xs"
            role="alert"
          >
            {{ field.errors[0] }}
          </p>
        </div>
      </FormischField>
    </div>
  </div>
</template>
