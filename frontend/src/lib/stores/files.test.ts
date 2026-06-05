import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { files, selectedIds, selectRange, loadDirectory } from './files';
import * as filesApi from '$lib/api/files';
import type { FileEntry } from '$lib/types/audio';

// Mock the API layer
vi.mock('$lib/api/files', () => ({
	listFiles: vi.fn()
}));

const mockFiles: FileEntry[] = [
	{ id: '1', filename: 'song1.mp3', relative_path: 'song1.mp3', size: 100, duration_secs: 120, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' },
	{ id: '2', filename: 'song2.mp3', relative_path: 'song2.mp3', size: 200, duration_secs: 180, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' },
	{ id: '3', filename: 'song3.mp3', relative_path: 'song3.mp3', size: 300, duration_secs: 200, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' },
	{ id: '4', filename: 'song4.mp3', relative_path: 'song4.mp3', size: 400, duration_secs: 220, format_label: 'MP3', format: 'mp3', has_cover: false, modified_at: '2026-06-05T00:00:00Z' }
];

describe('files store', () => {
	beforeEach(() => {
		files.set(mockFiles);
		selectedIds.set(new Set());
		vi.clearAllMocks();
	});

	describe('selectRange', () => {
		it('selects range over default files store', () => {
			selectRange('2', '4');
			const selected = get(selectedIds);
			expect(selected.size).toBe(3);
			expect(selected.has('2')).toBe(true);
			expect(selected.has('3')).toBe(true);
			expect(selected.has('4')).toBe(true);
			expect(selected.has('1')).toBe(false);
		});

		it('selects range over custom files array (sorted/filtered)', () => {
			// Custom order: 4, 2, 1, 3
			const customOrder = [mockFiles[3], mockFiles[1], mockFiles[0], mockFiles[2]];
			
			// Range from '2' (index 1) to '3' (index 3) should select '2', '1', and '3' (but NOT '4')
			selectRange('2', '3', customOrder);
			const selected = get(selectedIds);
			expect(selected.size).toBe(3);
			expect(selected.has('2')).toBe(true);
			expect(selected.has('1')).toBe(true);
			expect(selected.has('3')).toBe(true);
			expect(selected.has('4')).toBe(false);
		});
	});

	describe('loadDirectory generation guard', () => {
		it('prevents older in-flight loads from clobbering newer loads', async () => {
			let resolveFirst: (value: any) => void = () => {};
			let resolveSecond: (value: any) => void = () => {};

			const promiseFirst = new Promise((resolve) => { resolveFirst = resolve; });
			const promiseSecond = new Promise((resolve) => { resolveSecond = resolve; });

			vi.mocked(filesApi.listFiles)
				.mockImplementationOnce(() => promiseFirst as any)
				.mockImplementationOnce(() => promiseSecond as any);

			// Start first load (slow)
			const load1 = loadDirectory('/first');

			// Start second load (fast)
			const load2 = loadDirectory('/second');

			// Resolve second load first
			resolveSecond({
				files: [mockFiles[1]],
				total: 1,
				directories: ['dir2']
			});
			await load2;

			expect(get(files)).toEqual([mockFiles[1]]);

			// Now resolve first load (out of order)
			resolveFirst({
				files: [mockFiles[0]],
				total: 1,
				directories: ['dir1']
			});
			await load1;

			// Store should still reflect the second (newer) load's data
			expect(get(files)).toEqual([mockFiles[1]]);
		});
	});
});
