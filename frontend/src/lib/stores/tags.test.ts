import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { pendingEdits, loadedTags, saveAllEdits, setPendingEdit } from './tags';
import { files, selectedIds } from './files';
import * as tagsApi from '$lib/api/tags';
import type { FileEntry, TagData } from '$lib/types/audio';

// Mock tag and file APIs
vi.mock('$lib/api/tags', () => ({
	readTags: vi.fn().mockResolvedValue({}),
	readProperties: vi.fn().mockResolvedValue({}),
	writeTags: vi.fn()
}));

vi.mock('$lib/api/files', () => ({
	listFiles: vi.fn()
}));

const mockFiles: FileEntry[] = [
	{ id: 'file-1', filename: 'song1.mp3', relative_path: 'song1.mp3', size: 100, duration_secs: 120, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' },
	{ id: 'file-2', filename: 'song2.mp3', relative_path: 'song2.mp3', size: 200, duration_secs: 180, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' }
];

describe('tags store saveAllEdits', () => {
	beforeEach(() => {
		files.set(mockFiles);
		selectedIds.set(new Set(['file-1', 'file-2']));
		pendingEdits.set(new Map());
		loadedTags.set(new Map());
		vi.clearAllMocks();
	});

	it('clears all pending edits on successful save when there are no concurrent modifications', async () => {
		// Stage some edits
		setPendingEdit('title', 'New Title 1'); // file-1 and file-2 are selected

		expect(get(pendingEdits).get('file-1')).toEqual({ title: 'New Title 1' });
		expect(get(pendingEdits).get('file-2')).toEqual({ title: 'New Title 1' });

		vi.mocked(tagsApi.writeTags).mockResolvedValue([
			{ id: 'file-1', status: 'ok' },
			{ id: 'file-2', status: 'ok' }
		]);

		const res = await saveAllEdits();

		expect(res.success).toBe(2);
		expect(res.failed).toBe(0);
		expect(res.failedIds).toEqual([]);

		// pendingEdits should be completely cleared
		expect(get(pendingEdits).size).toBe(0);
		// loadedTags should be updated
		expect(get(loadedTags).get('file-1')).toEqual({ title: 'New Title 1' });
		expect(get(loadedTags).get('file-2')).toEqual({ title: 'New Title 1' });
	});

	it('preserves other fields edited concurrently while a save is in flight', async () => {
		pendingEdits.set(new Map([
			['file-1', { title: 'Saving Title' }]
		]));

		let resolveWrite: (val: any) => void = () => {};
		const writePromise = new Promise((resolve) => {
			resolveWrite = resolve;
		});
		vi.mocked(tagsApi.writeTags).mockImplementation(() => writePromise as any);

		// Start saving
		const savePromise = saveAllEdits();

		// Simulate user editing 'genre' concurrently while save is in flight
		pendingEdits.update((map) => {
			const next = new Map(map);
			const existing = next.get('file-1') || {};
			next.set('file-1', { ...existing, genre: 'New Genre' });
			return next;
		});

		// Resolve the save request successfully
		resolveWrite([
			{ id: 'file-1', status: 'ok' }
		]);

		const res = await savePromise;
		expect(res.success).toBe(1);

		// The title was saved, so it should be removed from pendingEdits.
		// The genre was not part of the in-flight save, so it must be preserved.
		const pending = get(pendingEdits);
		expect(pending.get('file-1')).toEqual({ genre: 'New Genre' });
	});

	it('preserves same fields edited concurrently to a different value while a save is in flight', async () => {
		pendingEdits.set(new Map([
			['file-1', { title: 'Saving Title' }]
		]));

		let resolveWrite: (val: any) => void = () => {};
		const writePromise = new Promise((resolve) => {
			resolveWrite = resolve;
		});
		vi.mocked(tagsApi.writeTags).mockImplementation(() => writePromise as any);

		// Start saving
		const savePromise = saveAllEdits();

		// Simulate user editing 'title' concurrently to a different value
		pendingEdits.update((map) => {
			const next = new Map(map);
			next.set('file-1', { title: 'New Staged Title' });
			return next;
		});

		// Resolve the save request successfully
		resolveWrite([
			{ id: 'file-1', status: 'ok' }
		]);

		const res = await savePromise;
		expect(res.success).toBe(1);

		// The new title value should be preserved since it was not yet saved.
		const pending = get(pendingEdits);
		expect(pending.get('file-1')).toEqual({ title: 'New Staged Title' });
	});

	it('handles partial failures by only clearing successfully saved file edits', async () => {
		pendingEdits.set(new Map([
			['file-1', { title: 'Title 1' }],
			['file-2', { title: 'Title 2' }]
		]));

		vi.mocked(tagsApi.writeTags).mockResolvedValue([
			{ id: 'file-1', status: 'ok' },
			{ id: 'file-2', status: 'error', error: 'Permission denied' }
		]);

		const res = await saveAllEdits();

		expect(res.success).toBe(1);
		expect(res.failed).toBe(1);
		expect(res.failedIds).toEqual(['file-2']);

		const pending = get(pendingEdits);
		expect(pending.has('file-1')).toBe(false);
		expect(pending.get('file-2')).toEqual({ title: 'Title 2' });
	});
});
