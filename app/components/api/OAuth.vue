<script setup lang="ts">
  import type { InferOutput } from 'valibot';
  import type { OAuthPending, OAuthToken } from '~/utils/oauthFlow';
  import {
    Field as FormischField,
    Form as FormischForm,
    getInput,
    submit,
    useForm,
  } from '@formisch/vue';
  import { invoke } from '@tauri-apps/api/core';
  import * as v from 'valibot';

  const oauthSchema = v.object({
    credential_id: v.pipe(
      v.string(),
      v.regex(/^[A-Za-z0-9_-]{1,40}$/, 'Use 1 to 40 letters, digits, underscores or hyphens.'),
    ),
    grant: v.picklist(['client_credentials', 'authorization_code']),
    authorization_endpoint: v.pipe(v.string(), v.maxLength(4096)),
    issuer: v.pipe(v.string(), v.maxLength(4096)),
    token_endpoint: v.pipe(
      v.string(),
      v.url('Enter the provider HTTPS token endpoint.'),
      v.startsWith('https://'),
    ),
    client_id: v.pipe(v.string(), v.nonEmpty('Enter a client ID.'), v.maxLength(4096)),
    client_secret_name: v.pipe(v.string(), v.maxLength(80)),
    scope: v.pipe(v.string(), v.maxLength(4096)),
  });
  const fields = [
    {
      name: 'credential_id',
      label: 'Credential ID',
      placeholder: 'development-api',
    },
    {
      name: 'authorization_endpoint',
      label: 'Authorization endpoint (PKCE)',
      placeholder: 'https://identity.example.com/authorize',
    },
    {
      name: 'issuer',
      label: 'Issuer identifier (required for PKCE)',
      placeholder: 'https://identity.example.com',
    },
    {
      name: 'token_endpoint',
      label: 'Token endpoint',
      placeholder: 'https://identity.example.com/token',
    },
    {
      name: 'client_id',
      label: 'Client ID',
      placeholder: '',
    },
    {
      name: 'client_secret_name',
      label: 'Client secret OS-store name (optional for PKCE)',
      placeholder: 'oauth-client-secret',
    },
    {
      name: 'scope',
      label: 'Scopes (space separated)',
      placeholder: 'read write',
    },
  ] as const;

  defineProps<{ disabled?: boolean }>();
  const emit = defineEmits<{ token: [reference: string] }>();
  const form = useForm({
    schema: oauthSchema,
    initialInput: {
      credential_id: '',
      grant: 'authorization_code',
      authorization_endpoint: '',
      issuer: '',
      token_endpoint: '',
      client_id: '',
      client_secret_name: '',
      scope: '',
    },
  });
  const pending = shallowRef<OAuthPending | null>(null);
  const busy = shallowRef<boolean>(false);
  const refresh = shallowRef<boolean>(false);
  const message = shallowRef<string | null>(null);
  let operation: AbortController | null = null;
  let revision = 0;

  async function acquire(values: InferOutput<typeof oauthSchema>): Promise<void> {
    const current = new AbortController();
    revision += 1;
    operation?.abort();
    operation = current;
    busy.value = true;
    message.value = null;
    try {
      const beginning = invoke<OAuthPending>('oauth_begin', {
        config: {
          ...values,
          issuer: values.issuer || null,
          authorization_endpoint: values.authorization_endpoint || null,
          client_secret_name: values.client_secret_name || null,
        },
        refresh: refresh.value,
      });
      const token = await receiveOAuthToken(
        beginning,
        (id) => invoke<OAuthToken>('oauth_complete', { id }),
        (id) => invoke('oauth_cancel', { id }).then(() => undefined),
        current.signal,
        (flow) => {
          pending.value = flow;
          message.value = flow.redirect_uri
            ? 'Finish authorization in your browser. Network and callback waits have a three-minute deadline. Cancellation waits for OS credential dialogs and cleanup to finish.'
            : 'Acquiring token from the provider.';
        },
      );
      if (!token || current.signal.aborted) {
        if (operation === current) {
          message.value = 'OAuth cancelled. No token reference was applied to this request.';
        }
        return;
      }
      emit('token', token.reference);
      message.value = `Token saved in the OS credential store.${token.expires_in === null ? ' Provider did not report a lifetime.' : ` Provider lifetime: ${token.expires_in} seconds.`} ${token.refresh_available ? 'Refresh is available through the explicit refresh action.' : 'The provider did not issue a refresh token.'}`;
    } catch (error) {
      if (operation === current) {
        message.value = formatAuthoringError(error);
      }
    } finally {
      if (operation === current) {
        operation = null;
        pending.value = null;
        busy.value = false;
      }
    }
  }

  function cancel(): void {
    operation?.abort();
    message.value =
      'Cancelling OAuth. Native cleanup must finish before another token operation can start.';
  }

  function deactivate(): void {
    revision += 1;
    if (operation) {
      message.value = 'OAuth cancellation requested after leaving this request.';
    }
    operation?.abort();
    operation = null;
    pending.value = null;
    busy.value = false;
  }

  function requestRefresh(): void {
    refresh.value = true;
    submit(form);
  }

  async function forget(): Promise<void> {
    const credentialId = getInput(form, { path: ['credential_id'] });
    if (!credentialId) {
      return;
    }
    const generation = ++revision;
    busy.value = true;
    try {
      await invoke('oauth_forget', { credentialId });
      if (revision === generation) {
        message.value = 'Local token deleted. This does not revoke the token at the provider.';
      }
    } catch (error) {
      if (revision === generation) {
        message.value = formatAuthoringError(error);
      }
    } finally {
      if (revision === generation) {
        busy.value = false;
      }
    }
  }

  onDeactivated(deactivate);
  onBeforeUnmount(deactivate);
</script>

<template>
  <section
    class="space-y-4 rounded-lg border p-5"
    aria-label="OAuth 2.0 credentials"
  >
    <h2 class="font-medium">OAuth 2.0</h2>

    <p class="text-muted text-sm">
      Acquire or refresh tokens explicitly. Velo stores tokens in the OS credential store and uses a
      Bearer secret reference in your request. PKCE opens your system browser and uses a temporary
      loopback callback with a random port. Register that loopback redirect with your provider. PKCE
      requires the provider's issuer identifier and an RFC 9207 issuer response parameter. Token
      requests never run during an active load test.
    </p>

    <FormischForm
      :of="form"
      class="space-y-4"
      @submit="acquire"
    >
      <FormischField
        v-slot="field"
        :of="form"
        :path="['grant']"
      >
        <div class="space-y-2">
          <label
            class="text-sm font-medium"
            for="oauth-grant"
          >
            Grant
          </label>

          <USelect
            v-model="field.input"
            :disabled="disabled || busy"
            :name="field.props.name"
            :items="[
              { label: 'Authorization code with PKCE', value: 'authorization_code' },
              { label: 'Client credentials', value: 'client_credentials' },
            ]"
            id="oauth-grant"
            class="w-full"
          />
        </div>
      </FormischField>

      <div class="grid gap-4 sm:grid-cols-2">
        <FormischField
          v-for="option in fields"
          v-slot="field"
          :key="option.name"
          :of="form"
          :path="[option.name]"
        >
          <div class="space-y-2">
            <label
              :for="`oauth-${option.name}`"
              class="text-sm font-medium"
            >
              {{ option.label }}
            </label>

            <UInput
              v-model="field.input"
              v-bind="field.props"
              :id="`oauth-${option.name}`"
              :disabled="disabled || busy"
              :placeholder="option.placeholder"
              :aria-invalid="Boolean(field.errors)"
              autocomplete="off"
              spellcheck="false"
            />

            <p
              v-if="field.errors"
              class="text-error text-xs"
              role="alert"
            >
              {{ field.errors.join(' ') }}
            </p>
          </div>
        </FormischField>
      </div>

      <div class="flex flex-wrap gap-2">
        <UButton
          :disabled="disabled || busy"
          type="submit"
          label="Acquire token"
          @click="refresh = false"
        />

        <UButton
          :disabled="disabled || busy"
          type="button"
          variant="outline"
          color="neutral"
          label="Refresh token"
          @click="requestRefresh"
        />

        <UButton
          :disabled="disabled || busy"
          type="button"
          variant="outline"
          color="neutral"
          label="Forget local token"
          @click="forget"
        />

        <UButton
          v-if="pending"
          type="button"
          variant="outline"
          color="neutral"
          label="Cancel OAuth"
          @click="cancel"
        />
      </div>
    </FormischForm>

    <p
      v-if="pending?.redirect_uri"
      class="text-muted font-mono text-xs break-all"
    >
      Callback: {{ pending.redirect_uri }}
    </p>

    <p
      v-if="message"
      class="text-sm"
      role="status"
    >
      {{ message }}
    </p>
  </section>
</template>
