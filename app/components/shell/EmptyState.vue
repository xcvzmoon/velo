<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
</script>

<template>
  <UEmpty
    :title="title"
    :description="description"
    icon="i-hugeicons-test-tube-01"
    variant="subtle"
  >
    <template
      v-if="$slots.default"
      #actions
    >
      <slot />
    </template>
  </UEmpty>
</template>
