import type { CollectionEntry } from '../../../app/types/contracts';
import { describe, expect, it } from 'vitest';
import {
  changeApiEntry,
  findApiEntry,
  newApiCollection,
  newApiRequest,
} from '../../../app/utils/apiCollections';

describe('API collection draft operations', () => {
  it('reorders nested requests without disturbing other folders or losing duplicate fields', () => {
    const first = newApiRequest('First');
    const second = newApiRequest('Second');
    if (first.kind !== 'request') {
      throw new Error('Expected a request');
    }
    first.definition.config.query = [
      {
        name: 'tag',
        value: 'one',
      },
      {
        name: 'tag',
        value: 'two',
      },
    ];
    const collection = newApiCollection('Orders');
    const folder: CollectionEntry = {
      kind: 'folder',
      id: 'folder',
      name: 'Folder',
      description: '',
      notes: '',
      defaults: {
        headers: [],
        auth: null,
        variables: {},
      },
      entries: [first, second],
    };
    collection.entries.push(folder, newApiRequest('Outside'));
    expect(changeApiEntry(collection.entries, second.id, 'up')).toBe(true);
    expect(folder.entries.map(({ name }) => name)).toEqual(['Second', 'First']);
    expect(findApiEntry(collection.entries, first.id)).toBe(first);
    expect(first.definition.config.query).toEqual([
      {
        name: 'tag',
        value: 'one',
      },
      {
        name: 'tag',
        value: 'two',
      },
    ]);
    expect(collection.entries[1]?.name).toBe('Outside');
  });
  it('duplicates a folder with fresh descendant identities and independent request values', () => {
    const original = newApiRequest('Get order');
    const folder: CollectionEntry = {
      kind: 'folder',
      id: 'folder',
      name: 'Orders',
      description: 'API usage',
      notes: 'Run after setup',
      defaults: {
        headers: [],
        auth: null,
        variables: { base: 'http://localhost' },
      },
      entries: [original],
    };
    const entries: CollectionEntry[] = [folder];
    expect(changeApiEntry(entries, folder.id, 'duplicate')).toBe(true);
    const copy = entries[1];
    expect(copy?.id).not.toBe(folder.id);
    expect(copy?.name).toBe('Orders copy');
    if (copy?.kind !== 'folder') {
      throw new Error('Expected a folder');
    }
    expect(copy.entries[0]?.id).not.toBe(original.id);
    copy.defaults.variables.base = 'http://example.test';
    expect(folder.defaults.variables.base).toBe('http://localhost');
    expect(copy.notes).toBe('Run after setup');
  });
  it('handles missing entries and boundary moves without losing contents', () => {
    const request = newApiRequest('Request');
    const entries = [request];
    expect(changeApiEntry(entries, request.id, 'up')).toBe(true);
    expect(changeApiEntry(entries, request.id, 'down')).toBe(true);
    expect(entries).toEqual([request]);
    expect(changeApiEntry(entries, 'missing', 'delete')).toBe(false);
    expect(findApiEntry(entries, 'missing')).toBeNull();
    changeApiEntry(entries, request.id, 'delete');
    expect(entries).toEqual([]);
  });
});
