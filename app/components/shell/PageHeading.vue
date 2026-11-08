<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
</script>

<template>
  <header class="flex flex-wrap items-start justify-between gap-4">
    <div>
      <h1 class="text-highlighted text-2xl font-semibold tracking-tight">{{ title }}</h1>
      <p class="text-muted mt-1 max-w-2xl text-sm leading-relaxed">{{ description }}</p>
    </div>

    <div
      v-if="$slots.default"
      class="flex flex-wrap gap-2"
    >
      <slot />
    </div>
  </header>
</template>
