<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
</script>

<template>
  <header class="mb-8 border-b pb-7">
    <p class="text-muted-foreground mb-3 font-mono text-[10px] tracking-[0.16em] uppercase">
      Velo / Studio
    </p>

    <h1 class="text-3xl font-medium tracking-tight">{{ title }}</h1>

    <p class="text-muted-foreground mt-3 max-w-xl text-sm leading-relaxed">{{ description }}</p>
  </header>
</template>
