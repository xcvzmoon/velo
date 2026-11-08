<script setup lang="ts">
  const colorMode = useColorMode();
  const isDark = computed<boolean>(() => colorMode.value === 'dark');
</script>

<template>
  <UButton
    :aria-label="`Switch to ${isDark ? 'light' : 'dark'} mode`"
    :icon="isDark ? 'i-hugeicons-moon-02' : 'i-hugeicons-sun-03'"
    size="sm"
    color="neutral"
    variant="ghost"
    class="p-1.5"
    @click="colorMode.preference = isDark ? 'light' : 'dark'"
  />
</template>
