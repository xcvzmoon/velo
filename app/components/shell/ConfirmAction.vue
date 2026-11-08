<script setup lang="ts">
  defineProps<{ label: string; title: string; description: string; disabled?: boolean }>();
  const emit = defineEmits<{ confirm: [] }>();
  const open = ref<boolean>(false);
  const trigger = useTemplateRef<{ $el: HTMLButtonElement }>('trigger');
  const cancelButton = useTemplateRef<{ $el: HTMLButtonElement }>('cancelButton');

  function restoreFocus(event: Event): void {
    event.preventDefault();
    trigger.value?.$el.focus();
  }

  function focusCancel(event: Event): void {
    event.preventDefault();
    cancelButton.value?.$el.focus();
  }
</script>

<template>
  <UModal
    v-model:open="open"
    :title="title"
    :description="description"
    :content="{ onOpenAutoFocus: focusCancel, onCloseAutoFocus: restoreFocus }"
  >
    <UButton
      ref="trigger"
      :disabled="disabled"
      :label="label"
      color="error"
      variant="ghost"
    />

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton
          ref="cancelButton"
          color="neutral"
          variant="outline"
          label="Cancel"
          @click="open = false"
        />

        <UButton
          :disabled="disabled"
          :label="label"
          color="error"
          @click="
            emit('confirm');
            open = false;
          "
        />
      </div>
    </template>
  </UModal>
</template>
