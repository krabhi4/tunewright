import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { applyReleaseToFiles } from './applyRelease';
import { pendingEdits, saveAllEdits } from '$lib/stores/tags';
import { executeRenames } from '$lib/api/rename';
import type { ReleaseDetail } from '$lib/api/lookup';
import type { FileEntry } from '$lib/types/audio';

// Mock the dependencies using a custom store implementation to avoid import hoisting issues.
vi.mock('$lib/stores/tags', () => {
	let value = new Map<string, any>();
	const subscribers = new Set<(val: any) => void>();
	const pendingEditsMock = {
		subscribe(fn: (val: any) => void) {
			subscribers.add(fn);
			fn(value);
			return () => {
				subscribers.delete(fn);
			};
		},
		set(newValue: any) {
			value = newValue;
			for (const sub of subscribers) {
				sub(value);
			}
		},
		update(fn: (val: any) => any) {
			this.set(fn(value));
		}
	};
	return {
		pendingEdits: pendingEditsMock,
		saveAllEdits: vi.fn()
	};
});

vi.mock('$lib/api/rename', () => ({
	executeRenames: vi.fn()
}));

describe('applyReleaseToFiles', () => {
	const mockRelease: ReleaseDetail = {
		id: 'release-1',
		title: 'Test Album',
		artist: 'Test Artist',
		year: 2026,
		genre: 'Synthwave',
		source: 'musicbrainz',
		cover_art_url: null,
		tracks: [
			{ title: 'Track 1', position: 1, duration_secs: 180, artist: 'Artist A' },
			{ title: 'Track 2', position: 2, duration_secs: 200, artist: null }
		]
	};

	const mockFile1: FileEntry = {
		id: 'file-1',
		filename: 'old1.mp3',
		relative_path: 'old1.mp3',
		size: 1000,
		duration_secs: 180,
		format_label: 'MP3',
		format: 'mp3',
		has_cover: false,
		modified_at: '2026-06-05T00:00:00Z'
	};

	const mockFile2: FileEntry = {
		id: 'file-2',
		filename: 'old2.mp3',
		relative_path: 'old2.mp3',
		size: 2000,
		duration_secs: 200,
		format_label: 'MP3',
		format: 'mp3',
		has_cover: false,
		modified_at: '2026-06-05T00:00:00Z'
	};

	beforeEach(() => {
		pendingEdits.set(new Map());
		vi.clearAllMocks();
	});

	it('stages release edits in pendingEdits store correctly', async () => {
		const matchedFiles = [mockFile1, mockFile2];
		
		await applyReleaseToFiles(mockRelease, matchedFiles, { rename: false });

		const pe = get(pendingEdits);
		expect(pe.size).toBe(2);

		expect(pe.get('file-1')).toEqual({
			title: 'Track 1',
			track_number: 1,
			album: 'Test Album',
			album_artist: 'Test Artist',
			year: 2026,
			genre: 'Synthwave',
			artist: 'Artist A'
		});

		expect(pe.get('file-2')).toEqual({
			title: 'Track 2',
			track_number: 2,
			album: 'Test Album',
			album_artist: 'Test Artist',
			year: 2026,
			genre: 'Synthwave',
			artist: 'Test Artist' // Falls back to release artist
		});

		// Ensure saveAllEdits & executeRenames were NOT called
		expect(saveAllEdits).not.toHaveBeenCalled();
		expect(executeRenames).not.toHaveBeenCalled();
	});

	it('handles null entries in matchedFiles gracefully', async () => {
		// Only match track 2, track 1 is null
		const matchedFiles = [null, mockFile2];

		await applyReleaseToFiles(mockRelease, matchedFiles, { rename: false });

		const pe = get(pendingEdits);
		expect(pe.size).toBe(1);
		expect(pe.has('file-1')).toBe(false);
		expect(pe.get('file-2')).toEqual({
			title: 'Track 2',
			track_number: 2,
			album: 'Test Album',
			album_artist: 'Test Artist',
			year: 2026,
			genre: 'Synthwave',
			artist: 'Test Artist'
		});
	});

	it('saves and renames successfully when rename is true and there are no failures', async () => {
		const matchedFiles = [mockFile1, mockFile2];

		vi.mocked(saveAllEdits).mockResolvedValue({ success: 2, failed: 0, failedIds: [] });
		vi.mocked(executeRenames).mockResolvedValue([
			{ id: 'file-1', status: 'ok', old_name: 'old1.mp3', new_name: '01 - Track 1.mp3', new_relative_path: '01 - Track 1.mp3' },
			{ id: 'file-2', status: 'ok', old_name: 'old2.mp3', new_name: '02 - Track 2.mp3', new_relative_path: '02 - Track 2.mp3' }
		]);

		const coverPaths = await applyReleaseToFiles(mockRelease, matchedFiles, { rename: true });

		expect(saveAllEdits).toHaveBeenCalled();
		expect(executeRenames).toHaveBeenCalledWith(
			[
				{ id: 'file-1', path: 'old1.mp3' },
				{ id: 'file-2', path: 'old2.mp3' }
			],
			'%track% - %title%'
		);
		expect(coverPaths).toEqual(['01 - Track 1.mp3', '02 - Track 2.mp3']);
	});

	it('skips renaming and filters coverPaths for files that fail saving', async () => {
		const matchedFiles = [mockFile1, mockFile2];

		// Suppose file-1 fails to save, but file-2 succeeds
		vi.mocked(saveAllEdits).mockResolvedValue({ success: 1, failed: 1, failedIds: ['file-1'] });
		vi.mocked(executeRenames).mockResolvedValue([
			{ id: 'file-2', status: 'ok', old_name: 'old2.mp3', new_name: '02 - Track 2.mp3', new_relative_path: '02 - Track 2.mp3' }
		]);

		const coverPaths = await applyReleaseToFiles(mockRelease, matchedFiles, { rename: true });

		expect(saveAllEdits).toHaveBeenCalled();
		// executeRenames should ONLY be called with file-2
		expect(executeRenames).toHaveBeenCalledWith(
			[{ id: 'file-2', path: 'old2.mp3' }],
			'%track% - %title%'
		);
		// coverPaths should ONLY contain the successfully saved and renamed/original path of file-2
		expect(coverPaths).toEqual(['02 - Track 2.mp3']);
	});

	it('handles the case where all files fail saving', async () => {
		const matchedFiles = [mockFile1, mockFile2];

		vi.mocked(saveAllEdits).mockResolvedValue({ success: 0, failed: 2, failedIds: ['file-1', 'file-2'] });

		const coverPaths = await applyReleaseToFiles(mockRelease, matchedFiles, { rename: true });

		expect(saveAllEdits).toHaveBeenCalled();
		// executeRenames should not be called at all
		expect(executeRenames).not.toHaveBeenCalled();
		// coverPaths should be empty since all files failed
		expect(coverPaths).toEqual([]);
	});

	it('handles rename execution failure gracefully', async () => {
		const matchedFiles = [mockFile1, mockFile2];

		vi.mocked(saveAllEdits).mockResolvedValue({ success: 2, failed: 0, failedIds: [] });
		vi.mocked(executeRenames).mockRejectedValue(new Error('Network error'));

		const coverPaths = await applyReleaseToFiles(mockRelease, matchedFiles, { rename: true });

		expect(saveAllEdits).toHaveBeenCalled();
		expect(executeRenames).toHaveBeenCalled();
		// Should fall back to the original paths of the successfully saved files
		expect(coverPaths).toEqual(['old1.mp3', 'old2.mp3']);
	});
});
