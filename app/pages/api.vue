<script setup lang="ts">
  definePageMeta({ keepalive: true });
</script>

<template>
  <div class="mx-auto max-w-7xl min-w-0 space-y-6">
    <ShellPageHeading
      title="API workspace"
      description="Organize HTTP requests, inspect responses, and run ordered functional checks."
    >
      <UButton
        to="/tests"
        label="Open test studio"
        icon="i-hugeicons-test-tube-01"
        color="neutral"
        variant="outline"
      />
    </ShellPageHeading>

    <ApiWorkspace />
  </div>
</template>
