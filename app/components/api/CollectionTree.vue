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
        <Button
          :variant="active === entry.id ? 'secondary' : 'ghost'"
          :disabled="disabled"
          class="h-auto max-w-full justify-start break-all whitespace-normal"
          @click="entry.kind === 'request' ? emit('open', entry.id) : emit('run', entry.id)"
        >
          {{ entry.kind === 'folder' ? 'Folder' : 'Request' }} · {{ entry.name }}
        </Button>

        <Button
          :disabled="disabled"
          :aria-label="`Move ${entry.name} up`"
          size="sm"
          variant="ghost"
          @click="emit('change', entry.id, 'up')"
        >
          ↑
        </Button>
        <Button
          :disabled="disabled"
          :aria-label="`Move ${entry.name} down`"
          size="sm"
          variant="ghost"
          @click="emit('change', entry.id, 'down')"
        >
          ↓
        </Button>
        <Button
          :disabled="disabled"
          size="sm"
          variant="ghost"
          @click="emit('change', entry.id, 'duplicate')"
        >
          Duplicate
        </Button>
        <Button
          :disabled="disabled"
          size="sm"
          variant="ghost"
          @click="emit('change', entry.id, 'delete')"
        >
          Delete
        </Button>
        <Button
          :disabled="disabled"
          :aria-pressed="selectedIds.includes(entry.id)"
          :variant="selectedIds.includes(entry.id) ? 'secondary' : 'outline'"
          size="sm"
          @click="emit('run', entry.id)"
        >
          {{ selectedIds.includes(entry.id) ? 'Selected for run' : 'Select for run' }}
        </Button>
      </div>

      <template v-if="entry.kind === 'folder'">
        <div class="flex gap-2">
          <Button
            :disabled="disabled"
            size="sm"
            variant="outline"
            @click="emit('add', entry.id, 'request')"
          >
            Add request
          </Button>
          <Button
            :disabled="disabled"
            size="sm"
            variant="outline"
            @click="emit('add', entry.id, 'folder')"
          >
            Add folder
          </Button>
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
