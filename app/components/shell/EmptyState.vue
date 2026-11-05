<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
</script>

<template>
  <section class="rounded-lg border border-dashed px-6 py-12">
    <div
      aria-hidden="true"
      class="bg-foreground/40 mb-6 h-px w-10"
    />

    <h2 class="text-base font-medium">{{ title }}</h2>

    <p class="text-muted-foreground mt-2 max-w-md text-sm leading-relaxed">{{ description }}</p>

    <div
      v-if="$slots.default"
      class="mt-6"
    >
      <slot />
    </div>
  </section>
</template>
