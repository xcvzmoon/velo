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
    {
      name: 'apiKeyName',
      label: 'API key name',
      kind: 'api_key',
    },
    {
      name: 'apiKeyValue',
      label: 'API key value',
      kind: 'api_key',
    },
  ] as const;

  defineProps<{
    form: FormStore<typeof requestEditorSchema>;
    disabled: boolean;
    hideTransport?: boolean;
  }>();
</script>

<template>
  <div class="border-default space-y-4 border-t pt-6">
    <h3 class="text-sm font-medium">Authentication and transport</h3>

    <p class="text-muted text-xs leading-relaxed">
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
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="request-auth"
            >
              Authentication
            </label>

            <USelect
              v-model="mode.input"
              :disabled="disabled"
              :name="mode.props.name"
              :aria-invalid="Boolean(mode.errors)"
              :items="[
                { label: 'None / custom header', value: 'none' },
                { label: 'Bearer token', value: 'bearer' },
                { label: 'Basic', value: 'basic' },
                { label: 'API key', value: 'api_key' },
              ]"
              id="request-auth"
              aria-describedby="request-auth-error"
              class="w-full"
            />
          </div>

          <p
            v-if="mode.errors"
            id="request-auth-error"
            class="text-error text-xs"
            role="alert"
          >
            {{ mode.errors[0] }}
          </p>
        </div>

        <FormischField
          v-if="mode.input === 'api_key'"
          v-slot="field"
          :of="form"
          :path="['apiKeyPlacement']"
        >
          <div class="max-w-xs space-y-2">
            <div class="grid min-w-0 gap-2">
              <label
                class="text-sm font-medium"
                for="api-key-placement"
              >
                API key placement
              </label>

              <USelect
                v-model="field.input"
                :disabled="disabled"
                :name="field.props.name"
                :items="[
                  { label: 'Header', value: 'header' },
                  { label: 'Query parameter', value: 'query' },
                ]"
                id="api-key-placement"
                class="w-full"
              />
            </div>

            <p class="text-muted text-xs">Query credentials may appear in server access logs.</p>
          </div>
        </FormischField>

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
                <div class="grid min-w-0 gap-2">
                  <label
                    :for="`request-${option.name}`"
                    class="text-sm font-medium"
                  >
                    {{ option.label }}
                  </label>

                  <UInput
                    v-model="field.input"
                    v-bind="field.props"
                    :id="`request-${option.name}`"
                    :type="
                      option.name === 'authUsername' || option.name === 'apiKeyName'
                        ? 'text'
                        : 'password'
                    "
                    autocomplete="off"
                    spellcheck="false"
                  />
                </div>
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
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="request-protocol"
            >
              HTTP protocol
            </label>

            <USelect
              v-model="field.input"
              :disabled="disabled"
              :name="field.props.name"
              :items="[
                { label: 'Negotiate HTTP/1.1 or HTTP/2', value: 'auto' },
                { label: 'Require HTTP/1.1', value: 'http1' },
                { label: 'Require HTTP/2', value: 'http2' },
              ]"
              id="request-protocol"
              class="w-full"
            />
          </div>
        </div>
      </FormischField>

      <FormischField
        v-slot="field"
        :of="form"
        :path="['max_redirects']"
      >
        <div class="space-y-2">
          <div class="grid min-w-0 gap-2">
            <label
              class="text-sm font-medium"
              for="request-redirects"
            >
              Maximum redirects (0 disables)
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :aria-invalid="Boolean(field.errors)"
              id="request-redirects"
              aria-describedby="request-redirects-error"
              type="number"
              min="0"
              step="1"
            />
          </div>

          <p
            v-if="field.errors"
            id="request-redirects-error"
            class="text-error text-xs"
            role="alert"
          >
            {{ field.errors[0] }}
          </p>
        </div>
      </FormischField>
    </div>
  </div>
</template>
