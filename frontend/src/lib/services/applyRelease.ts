import { pendingEdits, saveAllEdits } from '$lib/stores/tags';
import { executeRenames } from '$lib/api/rename';
import type { ReleaseDetail } from '$lib/api/lookup';
import type { FileEntry } from '$lib/types/audio';

/** Filename format used when renaming files to match a looked-up release. */
const RENAME_FORMAT = '%track% - %title%';

/**
 * Apply a looked-up release's track metadata to the positionally-matched files:
 * stage the tag edits, optionally save + rename, and return the (possibly
 * post-rename) relative paths that cover art should be embedded into.
 *
 * `matchedFiles[i]` is the file matched to `release.tracks[i]` (or `null`).
 */
export async function applyReleaseToFiles(
	release: ReleaseDetail,
	matchedFiles: (FileEntry | null)[],
	opts: { rename: boolean }
): Promise<string[]> {
	const tracks = release.tracks;
	const filesToRename: { id: string; path: string }[] = [];

	for (let i = 0; i < tracks.length; i++) {
		const file = matchedFiles[i];
		if (!file) continue;
		const track = tracks[i];

		pendingEdits.update((map) => {
			const next = new Map(map);
			const existing = next.get(file.id) || {};
			next.set(file.id, {
				...existing,
				title: track.title,
				track_number: track.position,
				album: release.title,
				album_artist: release.artist,
				...(release.year ? { year: release.year } : {}),
				...(release.genre ? { genre: release.genre } : {}),
				...(track.artist ? { artist: track.artist } : { artist: release.artist })
			});
			return next;
		});

		if (opts.rename) {
			filesToRename.push({ id: file.id, path: file.relative_path });
		}
	}

	const matched = matchedFiles.filter((f): f is FileEntry => f !== null);
	let coverPaths = matched.map((f) => f.relative_path);

	if (opts.rename && filesToRename.length > 0) {
		await saveAllEdits();
		try {
			const results = await executeRenames(filesToRename, RENAME_FORMAT);
			// Use the server-reported new path instead of re-deriving it client-side.
			const newPaths = new Map<string, string>();
			for (const res of results) {
				if (res.status === 'ok') newPaths.set(res.id, res.new_relative_path);
			}
			coverPaths = matched.map((f) => newPaths.get(f.id) ?? f.relative_path);
		} catch (err) {
			console.error('Rename failed:', err);
		}
	}

	return coverPaths;
}
