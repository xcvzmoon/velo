<script setup lang="ts">
  import type { CollectionEntry } from '~/types/contracts';

  const props = defineProps<{
    entries: CollectionEntry[];
    active: string | null;
    query: string;
    disabled: boolean;
    selectedIds: string[];
  }>();
  const emit = defineEmits<{
    open: [id: string];
    change: [id: string, action: 'duplicate' | 'delete' | 'up' | 'down'];
    add: [parent: string, kind: 'folder' | 'request'];
    run: [id: string];
  }>();
  function matches(entry: CollectionEntry): boolean {
    return (
      entry.name.toLowerCase().includes(props.query.toLowerCase()) ||
      (entry.kind === 'folder' && entry.entries.some(matches))
    );
  }
  const visible = computed<CollectionEntry[]>(() => props.entries.filter(matches));
</script>

<template>
  <ul
    class="space-y-2"
    aria-label="Ordered collection contents"
  >
    <li
      v-for="entry in visible"
      :key="entry.id"
      class="space-y-2 border-l pl-3"
    >
      <div class="flex flex-wrap items-center gap-1">
        <UButton
          :variant="active === entry.id ? 'soft' : 'ghost'"
          :disabled="disabled"
          :label="`${entry.kind === 'folder' ? 'Folder' : 'Request'} · ${entry.name}`"
          class="h-auto max-w-full justify-start break-all whitespace-normal"
          @click="entry.kind === 'request' ? emit('open', entry.id) : emit('run', entry.id)"
        />

        <UButton
          :disabled="disabled"
          :aria-label="`Move ${entry.name} up`"
          size="sm"
          variant="ghost"
          color="neutral"
          label="↑"
          @click="emit('change', entry.id, 'up')"
        />

        <UButton
          :disabled="disabled"
          :aria-label="`Move ${entry.name} down`"
          size="sm"
          variant="ghost"
          color="neutral"
          label="↓"
          @click="emit('change', entry.id, 'down')"
        />

        <UButton
          :disabled="disabled"
          size="sm"
          variant="ghost"
          color="neutral"
          label="Duplicate"
          @click="emit('change', entry.id, 'duplicate')"
        />

        <ShellConfirmAction
          :disabled="disabled"
          label="Delete"
          title="Delete collection entry?"
          description="This removes the selected entry and any requests inside it."
          @confirm="emit('change', entry.id, 'delete')"
        />

        <UButton
          :disabled="disabled"
          :aria-pressed="selectedIds.includes(entry.id)"
          :variant="selectedIds.includes(entry.id) ? 'soft' : 'outline'"
          :label="selectedIds.includes(entry.id) ? 'Selected for run' : 'Select for run'"
          size="sm"
          @click="emit('run', entry.id)"
        />
      </div>

      <template v-if="entry.kind === 'folder'">
        <div class="flex gap-2">
          <UButton
            :disabled="disabled"
            size="sm"
            variant="outline"
            color="neutral"
            label="Add request"
            @click="emit('add', entry.id, 'request')"
          />

          <UButton
            :disabled="disabled"
            size="sm"
            variant="outline"
            color="neutral"
            label="Add folder"
            @click="emit('add', entry.id, 'folder')"
          />
        </div>

        <ApiCollectionTree
          :entries="entry.entries"
          :active="active"
          :query="query"
          :selected-ids="selectedIds"
          :disabled="disabled"
          @open="emit('open', $event)"
          @change="(id, action) => emit('change', id, action)"
          @add="(parent, kind) => emit('add', parent, kind)"
          @run="emit('run', $event)"
        />
      </template>
    </li>
  </ul>
</template>
