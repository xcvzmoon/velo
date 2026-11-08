import type { ApiCollection, CollectionEntry, TestDefinition } from '../types/contracts';
import { toRaw } from 'vue';
/**
 * Finds an entry without changing the collection's displayed order.
 * @param entries - Ordered collection or folder contents.
 * @param id - Stable entry identifier.
 * @returns The matching entry, or null when it no longer exists.
 * @example
 * ```ts
 * const entry = findApiEntry(collection.entries, requestId)
 * ```
 */
export function findApiEntry(entries: CollectionEntry[], id: string): CollectionEntry | null {
  for (const entry of entries) {
    if (entry.id === id) {
      return entry;
    }
    if (entry.kind === 'folder') {
      const found = findApiEntry(entry.entries, id);
      if (found) {
        return found;
      }
    }
  }
  return null;
}
/**
 * Changes one entry in place while retaining sibling and descendant order.
 * @param entries - Mutable draft contents.
 * @param id - Stable entry identifier.
 * @param action - Replacement, duplication, removal, or one-position movement.
 * @returns Whether the entry was found.
 * @example
 * ```ts
 * changeApiEntry(collection.entries, requestId, 'up')
 * ```
 */
export function changeApiEntry(
  entries: CollectionEntry[],
  id: string,
  action: 'duplicate' | 'delete' | 'up' | 'down',
): boolean {
  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index];
    if (!entry) {
      continue;
    }
    if (entry.id === id) {
      if (action === 'delete') {
        entries.splice(index, 1);
      } else if (action === 'duplicate') {
        const duplicate = structuredClone(toRaw(entry));
        reidentify(duplicate);
        duplicate.name += ' copy';
        entries.splice(index + 1, 0, duplicate);
      } else {
        const destination = action === 'up' ? index - 1 : index + 1;
        if (destination >= 0 && destination < entries.length) {
          entries.splice(index, 1);
          entries.splice(destination, 0, entry);
        }
      }
      return true;
    }
    if (entry.kind === 'folder' && changeApiEntry(entry.entries, id, action)) {
      return true;
    }
  }
  return false;
}
function reidentify(entry: CollectionEntry): void {
  entry.id = crypto.randomUUID();
  if (entry.kind === 'folder') {
    for (const child of entry.entries) {
      reidentify(child);
    }
  }
}
/**
 * Creates an empty local collection with no credentials or traffic.
 * @param name - User-visible collection name.
 * @returns A versioned collection ready for editing.
 * @example
 * ```ts
 * const collection = newApiCollection('Orders')
 * ```
 */
export function newApiCollection(name: string): ApiCollection {
  return {
    version: 1,
    id: crypto.randomUUID(),
    name,
    description: '',
    notes: '',
    defaults: {
      headers: [],
      auth: null,
      variables: {},
    },
    environments: {},
    sensitive_response_fields: [],
    entries: [],
  };
}
/**
 * Creates a request using the shared test format and safe engine defaults.
 * @param name - Request name.
 * @returns An unsent reusable request.
 * @example
 * ```ts
 * const request = newApiRequest('List orders')
 * ```
 */
export function newApiRequest(name: string): CollectionEntry {
  const definition: TestDefinition = {
    version: 1,
    variables: {},
    environments: {},
    config: {
      url: 'http://127.0.0.1:8080/',
      method: 'GET',
      headers: [],
      query: [],
      body: { kind: 'none' },
      auth: { kind: 'none' },
      protocol: 'auto',
      max_redirects: 0,
      workload: {
        kind: 'constant',
        vus: 1,
        duration_ms: 1000,
      },
      timeout_ms: 30000,
      pacing_ms: 0,
      grace_ms: 5000,
      warmup_ms: 0,
      thresholds: [],
      abort_policies: [],
      assertions: [],
      inspection: {
        body_bytes: 16384,
        sample_count: 16,
        sample_bytes: 8192,
      },
      budget: {
        max_vus: 256,
        max_request_bytes: 8388608,
      },
    },
  };
  return {
    kind: 'request',
    id: crypto.randomUUID(),
    name,
    description: '',
    notes: '',
    inherit_auth: true,
    definition,
    extractions: [],
  };
}
