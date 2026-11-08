<script setup lang="ts">
  const styles = {
    card: {
      root: 'bg-default ring-default rounded-xl shadow-sm',
      header: 'bg-elevated/50 rounded-t-xl p-4 sm:px-5',
      body: 'p-4 sm:p-5',
      footer: 'bg-elevated/30 p-4 sm:px-5',
    },
    section: {
      root: 'divide-y-0 overflow-visible rounded-none bg-transparent shadow-none',
      header: 'p-0 pb-4 sm:px-0',
      body: 'p-0 sm:p-0',
      footer: 'p-0 pt-4 sm:px-0',
    },
    inset: {
      root: 'bg-elevated/40 divide-y-0 rounded-lg shadow-none',
      header: 'p-4 pb-0 sm:px-4',
      body: 'p-4 sm:p-4',
      footer: 'p-4 pt-0 sm:px-4',
    },
  };

  const { variant = 'card' } = defineProps<{ variant?: 'card' | 'section' | 'inset' }>();
</script>

<template>
  <UCard
    :variant="variant === 'card' ? 'outline' : 'soft'"
    :ui="{
      root: styles[variant].root,
      header: styles[variant].header,
      body: ['min-w-0 space-y-4', styles[variant].body],
      footer: styles[variant].footer,
    }"
    as="section"
    class="min-w-0"
  >
    <template
      v-if="$slots.header"
      #header
    >
      <slot name="header" />
    </template>

    <slot />

    <template
      v-if="$slots.footer"
      #footer
    >
      <slot name="footer" />
    </template>
  </UCard>
</template>
