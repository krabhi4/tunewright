import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	pendingEdits,
	loadedTags,
	saveAllEdits,
	setPendingEdit,
	clearPendingEdit,
	queuePropertiesFetch,
	selectedTags,
	hasPendingEdits,
	KEEP_VALUE
} from './tags';
import { files, selectedIds } from './files';
import { auth } from './auth';
import { toasts } from './toast';
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

describe('tags store write semantics', () => {
	beforeEach(() => {
		files.set(mockFiles);
		selectedIds.set(new Set(['file-1']));
		pendingEdits.set(new Map());
		loadedTags.set(new Map());
		vi.clearAllMocks();
	});

	it('sends null for a cleared field so the server removes it', async () => {
		setPendingEdit('year', null);
		vi.mocked(tagsApi.writeTags).mockResolvedValue([{ id: 'file-1', status: 'ok' }]);

		await saveAllEdits();

		const body = JSON.parse(JSON.stringify(vi.mocked(tagsApi.writeTags).mock.calls[0][0]));
		expect(body[0].tags).toEqual({ year: null });
	});

	it('shares one in-flight write between concurrent saves', async () => {
		setPendingEdit('title', 'Once');
		vi.mocked(tagsApi.writeTags).mockResolvedValue([{ id: 'file-1', status: 'ok' }]);

		const [a, b] = await Promise.all([saveAllEdits(), saveAllEdits()]);

		expect(tagsApi.writeTags).toHaveBeenCalledTimes(1);
		expect(a).toEqual(b);
	});

	it('queues a second write when edits are staged during an in-flight save', async () => {
		setPendingEdit('title', 'First');
		let resolveFirst: (val: any) => void = () => {};
		vi.mocked(tagsApi.writeTags)
			.mockImplementationOnce(() => new Promise((resolve) => (resolveFirst = resolve)) as any)
			.mockResolvedValueOnce([{ id: 'file-1', status: 'ok' }]);

		const first = saveAllEdits();
		setPendingEdit('genre', 'Later');
		const second = saveAllEdits();
		expect(second).not.toBe(first);

		resolveFirst([{ id: 'file-1', status: 'ok' }]);
		await second;

		expect(tagsApi.writeTags).toHaveBeenCalledTimes(2);
		expect(vi.mocked(tagsApi.writeTags).mock.calls[1][0][0].tags).toEqual({ genre: 'Later' });
		expect(get(pendingEdits).size).toBe(0);
	});

	it('drops pending edits for files missing from the listing', async () => {
		pendingEdits.set(new Map([['gone', { title: 'Orphan' }]]));

		const res = await saveAllEdits();

		expect(res).toEqual({ success: 0, failed: 0, failedIds: [] });
		expect(tagsApi.writeTags).not.toHaveBeenCalled();
		expect(get(hasPendingEdits)).toBe(false);
		expect(get(toasts).at(-1)?.message).toContain('1 file(s)');
	});

	it('keeps pending edits when the listing is empty', async () => {
		files.set([]);
		pendingEdits.set(new Map([['file-1', { title: 'Kept' }]]));

		const res = await saveAllEdits();

		expect(res).toEqual({ success: 0, failed: 1, failedIds: ['file-1'] });
		expect(tagsApi.writeTags).not.toHaveBeenCalled();
		expect(get(pendingEdits).get('file-1')).toEqual({ title: 'Kept' });
	});

	it('properties backfill only merges audio properties into loaded tags', async () => {
		vi.useFakeTimers();
		try {
			loadedTags.set(new Map([['file-1', { title: 'Saved' } as TagData]]));
			vi.mocked(tagsApi.readProperties).mockResolvedValue({
				'file-1': { title: 'Stale', bitrate: 320, duration_secs: 120 }
			});

			queuePropertiesFetch(['file-1']);
			await vi.runAllTimersAsync();

			expect(get(loadedTags).get('file-1')).toMatchObject({ title: 'Saved', bitrate: 320, duration_secs: 120 });
		} finally {
			vi.useRealTimers();
		}
	});
});

describe('tags store mixed values', () => {
    beforeEach(() => {
        files.set(mockFiles);
        selectedIds.set(new Set(['file-1', 'file-2']));
        pendingEdits.set(new Map());
        loadedTags.set(new Map([
            ['file-1', { title: 'Same', artist: 'A', year: 2001 } as TagData],
            ['file-2', { title: 'Same', artist: 'B', year: 2002 } as TagData]
        ]));
        vi.clearAllMocks();
    });

    it('shows keep for mixed numeric fields', () => {
        expect(get(selectedTags)).toMatchObject({ title: 'Same', artist: KEEP_VALUE, year: KEEP_VALUE });
    });

    it('leaves a mixed field unchanged when it is emptied', () => {
        setPendingEdit('year', 1999);
        setPendingEdit('artist', 'C');
        clearPendingEdit('year');
        expect(get(pendingEdits).get('file-1')).toEqual({ artist: 'C' });

        clearPendingEdit('artist');
        expect(get(pendingEdits).size).toBe(0);
        expect(get(selectedTags)).toMatchObject({ artist: KEEP_VALUE, year: KEEP_VALUE });
    });

    it('records nothing when emptying a field that is already empty', () => {
        clearPendingEdit('genre');
        expect(get(pendingEdits).size).toBe(0);
    });

    it('removes a field with a common value when it is emptied', () => {
        clearPendingEdit('title');
        expect(get(pendingEdits).get('file-1')).toEqual({ title: null });
        expect(get(pendingEdits).get('file-2')).toEqual({ title: null });
    });
});

describe('tags store edit ownership', () => {
	const user = (username: string) => ({
		checked: true,
		setupRequired: false,
		authenticated: true,
		user: { username, role: 'admin' as const }
	});

	beforeEach(() => {
		auth.set(user('alice'));
		pendingEdits.set(new Map([['file-1', { title: 'Draft' }]]));
	});

	it('keeps pending edits when the same user logs back in', () => {
		auth.set({ ...user('alice'), authenticated: false, user: null });
		auth.set(user('alice'));
		expect(get(pendingEdits).get('file-1')).toEqual({ title: 'Draft' });
	});

	it('clears pending edits when a different user logs in', () => {
		auth.set({ ...user('alice'), authenticated: false, user: null });
		auth.set(user('bob'));
		expect(get(pendingEdits).size).toBe(0);
	});
});
