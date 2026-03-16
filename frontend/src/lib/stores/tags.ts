import { writable, derived, get } from 'svelte/store';
import type { TagData } from '$lib/types/audio';
import { readTags, writeTags } from '$lib/api/tags';
import { files, selectedIds } from './files';

// Tags loaded from server, keyed by file ID
export const loadedTags = writable<Map<string, TagData>>(new Map());

// Pending edits not yet saved, keyed by file ID -> partial tag data
export const pendingEdits = writable<Map<string, Partial<TagData>>>(new Map());

// Whether we have unsaved changes
export const hasPendingEdits = derived(pendingEdits, ($pe) => $pe.size > 0);

// Merged view: loaded + pending overlay
export const mergedTags = derived([loadedTags, pendingEdits], ([$loaded, $pending]) => {
	const result = new Map<string, TagData>();
	for (const [id, tags] of $loaded) {
		const edits = $pending.get(id);
		if (edits) {
			result.set(id, { ...tags, ...edits });
		} else {
			result.set(id, tags);
		}
	}
	return result;
});

// Tags for currently selected files, with intersection logic
export const selectedTags = derived(
	[mergedTags, selectedIds],
	([$merged, $selected]) => {
		const ids = Array.from($selected);
		if (ids.length === 0) return null;

		const tagsList = ids.map((id) => $merged.get(id)).filter(Boolean) as TagData[];
		if (tagsList.length === 0) return null;

		if (tagsList.length === 1) return tagsList[0];

		// Intersection: find common values
		return intersectTags(tagsList);
	}
);

const TAG_FIELDS = [
	'title', 'artist', 'album', 'album_artist', 'genre', 'comment', 'composer'
] as const;

const TAG_NUMBER_FIELDS = [
	'year', 'track_number', 'track_total', 'disc_number', 'disc_total'
] as const;

export const KEEP_VALUE = '< keep >';

function intersectTags(tagsList: TagData[]): TagData {
	const result: TagData = {};

	for (const field of TAG_FIELDS) {
		const values = tagsList.map((t) => t[field] ?? '');
		const allSame = values.every((v) => v === values[0]);
		(result as any)[field] = allSame ? values[0] || undefined : KEEP_VALUE;
	}

	for (const field of TAG_NUMBER_FIELDS) {
		const values = tagsList.map((t) => t[field]);
		const allSame = values.every((v) => v === values[0]);
		(result as any)[field] = allSame ? values[0] : undefined;
	}

	return result;
}

// Fetch tags for a set of file IDs
export async function fetchTagsForFiles(ids: string[]) {
	const $files = get(files);
	const $loaded = get(loadedTags);

	// Only fetch for files we don't already have
	const needed = ids.filter((id) => !$loaded.has(id));
	if (needed.length === 0) return;

	// Build id -> relative_path map
	const paths: Record<string, string> = {};
	for (const id of needed) {
		const file = $files.find((f) => f.id === id);
		if (file) paths[id] = file.relative_path;
	}

	if (Object.keys(paths).length === 0) return;

	try {
		const tags = await readTags(needed, paths);
		loadedTags.update((map) => {
			const next = new Map(map);
			for (const [id, data] of Object.entries(tags)) {
				next.set(id, data);
			}
			return next;
		});
	} catch (err) {
		console.error('Failed to fetch tags:', err);
	}
}

// Set a pending edit for a field on all currently selected files
export function setPendingEdit(field: string, value: string | number | undefined) {
	const $selected = get(selectedIds);
	if ($selected.size === 0) return;

	pendingEdits.update((map) => {
		const next = new Map(map);
		for (const id of $selected) {
			const existing = next.get(id) || {};
			next.set(id, { ...existing, [field]: value });
		}
		return next;
	});
}

// Save all pending edits to the server
export async function saveAllEdits(): Promise<{ success: number; failed: number }> {
	const $pending = get(pendingEdits);
	const $files = get(files);

	if ($pending.size === 0) return { success: 0, failed: 0 };

	const changes = Array.from($pending.entries()).map(([id, edits]) => {
		const file = $files.find((f) => f.id === id);
		return {
			id,
			path: file?.relative_path ?? '',
			tags: edits
		};
	}).filter((c) => c.path !== '');

	try {
		const results = await writeTags(changes);

		let success = 0;
		let failed = 0;

		pendingEdits.update((map) => {
			const next = new Map(map);
			for (const r of results) {
				if (r.status === 'ok') {
					next.delete(r.id);
					success++;
					// Update loaded tags with the saved values
					loadedTags.update((loaded) => {
						const updated = new Map(loaded);
						const current = updated.get(r.id) || {};
						const edits = $pending.get(r.id) || {};
						updated.set(r.id, { ...current, ...edits });
						return updated;
					});
				} else {
					failed++;
					console.error(`Failed to save ${r.id}: ${r.error}`);
				}
			}
			return next;
		});

		return { success, failed };
	} catch (err) {
		console.error('Failed to save tags:', err);
		return { success: 0, failed: $pending.size };
	}
}

// Discard all pending edits
export function discardEdits() {
	pendingEdits.set(new Map());
}

// Clear all loaded tags (e.g., when changing directory)
export function clearTags() {
	loadedTags.set(new Map());
	pendingEdits.set(new Map());
}
