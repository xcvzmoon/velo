<script setup lang="ts">
  definePageMeta({ keepalive: true });
</script>

<template>
  <div class="space-y-8">
    <ShellPageHeading
      title="API workspace"
      description="Organize HTTP requests, inspect responses, and run ordered functional checks."
    />
    <ApiWorkspace />
  </div>
</template>
