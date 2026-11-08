<script setup lang="ts">
  defineProps<{ title: string; description: string }>();
  const emit = defineEmits<{ close: [confirmed: boolean] }>();
  const cancelButton = useTemplateRef<{ $el: HTMLButtonElement }>('cancelButton');

  function focusCancel(event: Event): void {
    event.preventDefault();
    cancelButton.value?.$el.focus();
  }
</script>

<template>
  <UModal
    :title="title"
    :description="description"
    :close="{ onClick: () => emit('close', false) }"
    :content="{ onOpenAutoFocus: focusCancel }"
    :ui="{ footer: 'justify-end' }"
  >
    <template #footer>
      <UButton
        ref="cancelButton"
        label="Cancel"
        color="neutral"
        variant="outline"
        @click="emit('close', false)"
      />

      <UButton
        label="Confirm"
        color="neutral"
        @click="emit('close', true)"
      />
    </template>
  </UModal>
</template>
