<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
</script>

<template>
  <header class="mb-6 flex flex-wrap items-start justify-between gap-4 border-b pb-6">
    <div>
      <h1 class="text-highlighted text-2xl font-semibold tracking-tight">{{ title }}</h1>
      <p class="text-muted mt-2 max-w-2xl text-sm leading-relaxed">{{ description }}</p>
    </div>

    <div
      v-if="$slots.default"
      class="flex flex-wrap gap-2"
    >
      <slot />
    </div>
  </header>
</template>
