import { writable, derived, get } from 'svelte/store';
import type { TagData } from '$lib/types/audio';
import { readTags, readProperties, writeTags } from '$lib/api/tags';
import { filesById, selectedIds } from './files';

// Tags loaded from server, keyed by file ID
export const loadedTags = writable<Map<string, TagData>>(new Map());

// Pending edits not yet saved, keyed by file ID -> partial tag data
export const pendingEdits = writable<Map<string, Partial<TagData>>>(new Map());

// Whether we have unsaved changes
export const hasPendingEdits = derived(pendingEdits, ($pe) => $pe.size > 0);

// Number of files with unsaved edits
export const pendingEditCount = derived(pendingEdits, ($pe) => $pe.size);

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

// Generation counter — incremented on clearTags() to invalidate in-flight fetches
let fetchGeneration = 0;

// Fetch tags for a set of file IDs
export async function fetchTagsForFiles(ids: string[], force = false) {
	const $filesById = get(filesById);
	const $loaded = get(loadedTags);
	const gen = fetchGeneration;

	// Only fetch for files we don't already have (unless forced)
	const needed = force ? ids : ids.filter((id) => !$loaded.has(id));
	if (needed.length === 0) return;

	// Build id -> relative_path map
	const paths: Record<string, string> = {};
	for (const id of needed) {
		const file = $filesById.get(id);
		if (file) paths[id] = file.relative_path;
	}

	if (Object.keys(paths).length === 0) return;

	try {
		const tags = await readTags(needed, paths);
		// Discard if directory changed while fetching
		if (gen !== fetchGeneration) return;
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

// Track which files have had properties loaded
const propertiesLoaded = new Set<string>();

// Fetch audio properties (duration, bitrate) for files that already have fast tags.
// Called as a background backfill after the grid is populated.
let propertiesTimer: ReturnType<typeof setTimeout> | null = null;
let pendingPropertyIds: string[] = [];

function processNextPropertiesBatch() {
	const batch = pendingPropertyIds.splice(0, 50);
	if (batch.length > 0) {
		fetchPropertiesForFiles(batch);
	}
	if (pendingPropertyIds.length > 0) {
		propertiesTimer = setTimeout(processNextPropertiesBatch, 50);
	} else {
		propertiesTimer = null;
	}
}

export function queuePropertiesFetch(ids: string[]) {
	const needed = ids.filter((id) => !propertiesLoaded.has(id));
	if (needed.length === 0) return;
	pendingPropertyIds = [...new Set([...pendingPropertyIds, ...needed])];

	if (propertiesTimer) clearTimeout(propertiesTimer);
	propertiesTimer = setTimeout(processNextPropertiesBatch, 200);
}

async function fetchPropertiesForFiles(ids: string[]) {
	const $filesById = get(filesById);
	const gen = fetchGeneration;

	const paths: Record<string, string> = {};
	for (const id of ids) {
		const file = $filesById.get(id);
		if (file) paths[id] = file.relative_path;
	}
	if (Object.keys(paths).length === 0) return;

	try {
		const tags = await readProperties(ids, paths);
		// Discard if directory changed while fetching
		if (gen !== fetchGeneration) return;
		loadedTags.update((map) => {
			const next = new Map(map);
			for (const [id, data] of Object.entries(tags)) {
				const existing = next.get(id);
				// Merge: keep existing tag fields, add audio properties
				next.set(id, { ...existing, ...data });
				propertiesLoaded.add(id);
			}
			return next;
		});
	} catch (err) {
		console.error('Failed to fetch properties:', err);
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
export async function saveAllEdits(): Promise<{ success: number; failed: number; failedIds: string[] }> {
	const $pending = get(pendingEdits);
	const $filesById = get(filesById);

	if ($pending.size === 0) return { success: 0, failed: 0, failedIds: [] };

	const changes = Array.from($pending.entries()).map(([id, edits]) => {
		const file = $filesById.get(id);
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
		const failedIds: string[] = [];
		const loadedUpdates: Array<[string, TagData]> = [];

		pendingEdits.update((map) => {
			const next = new Map(map);
			for (const r of results) {
				if (r.status === 'ok') {
					success++;
					const current = get(loadedTags).get(r.id) || {};
					const edits = $pending.get(r.id) || {};
					loadedUpdates.push([r.id, { ...current, ...edits } as TagData]);

					const liveEdits = next.get(r.id);
					if (liveEdits) {
						const nextEdits = { ...liveEdits };
						for (const key of Object.keys(edits)) {
							const k = key as keyof TagData;
							if (nextEdits[k] === edits[k]) {
								delete nextEdits[k];
							}
						}
						if (Object.keys(nextEdits).length === 0) {
							next.delete(r.id);
						} else {
							next.set(r.id, nextEdits);
						}
					}
				} else {
					failed++;
					failedIds.push(r.id);
					console.error(`Failed to save ${r.id}: ${r.error}`);
				}
			}
			return next;
		});

		// Apply optimistic update after pendingEdits is settled (avoids nested store update)
		if (loadedUpdates.length > 0) {
			loadedTags.update((loaded) => {
				const next = new Map(loaded);
				for (const [id, data] of loadedUpdates) next.set(id, data);
				return next;
			});
		}

		// Re-read saved files from disk to confirm actual state
		const savedIds = results.filter((r) => r.status === 'ok').map((r) => r.id);
		if (savedIds.length > 0) {
			await fetchTagsForFiles(savedIds, true);
		}

		return { success, failed, failedIds };
	} catch (err) {
		console.error('Failed to save tags:', err);
		return { success: 0, failed: $pending.size, failedIds: Array.from($pending.keys()) };
	}
}

// Discard all pending edits
export function discardEdits() {
	pendingEdits.set(new Map());
}

// Clear all loaded tags (e.g., when changing directory)
export function clearTags() {
	fetchGeneration++; // invalidate any in-flight fetches
	loadedTags.set(new Map());
	pendingEdits.set(new Map());
	propertiesLoaded.clear();
	pendingPropertyIds = [];
	if (propertiesTimer) {
		clearTimeout(propertiesTimer);
		propertiesTimer = null;
	}
}
