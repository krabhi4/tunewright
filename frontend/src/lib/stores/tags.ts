import { writable, derived, get } from 'svelte/store';
import type { TagData, TagEdits } from '$lib/types/audio';
import type { ApiError } from '$lib/api/client';
import { readTags, readProperties, writeTags } from '$lib/api/tags';
import { filesById, selectedIds } from './files';
import { auth } from './auth';
import { toast } from './toast';

// Tags loaded from server, keyed by file ID
export const loadedTags = writable<Map<string, TagData>>(new Map());

// Pending edits not yet saved, keyed by file ID -> partial tag data
export const pendingEdits = writable<Map<string, TagEdits>>(new Map());

// Whether we have unsaved changes
export const hasPendingEdits = derived(pendingEdits, ($pe) => $pe.size > 0);

// Number of files with unsaved edits
export const pendingEditCount = derived(pendingEdits, ($pe) => $pe.size);

let editsOwner: string | null = null;
auth.subscribe(($auth) => {
	const username = $auth.user?.username;
	if (!username) return;
	if (editsOwner !== null && editsOwner !== username) pendingEdits.set(new Map());
	editsOwner = username;
});

// Merged view: loaded + pending overlay
export const mergedTags = derived([loadedTags, pendingEdits], ([$loaded, $pending]) => {
	if ($pending.size === 0) return $loaded;
	const result = new Map($loaded);
	for (const [id, edits] of $pending) {
		const tags = result.get(id);
		if (tags) result.set(id, { ...tags, ...edits } as TagData);
	}
	return result;
});

// Tags for currently selected files, with intersection logic
const unreadableIds = writable<Set<string>>(new Set());

export const selectedUnreadableCount = derived(
	[selectedIds, unreadableIds],
	([$selected, $unreadable]) => Array.from($selected).filter((id) => $unreadable.has(id)).length
);

export const selectedTags = derived(
	[mergedTags, selectedIds, unreadableIds],
	([$merged, $selected, $unreadable]) => {
		const ids = Array.from($selected).filter((id) => !$unreadable.has(id));
		if (ids.length === 0) return null;

		const tagsList = ids.map((id) => $merged.get(id)).filter(Boolean) as TagData[];
		if (tagsList.length === 0) return null;

		if (tagsList.length < ids.length) return intersectTags([...tagsList, NOT_LOADED]);
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

const NOT_LOADED = Object.fromEntries([
	...TAG_FIELDS.map((f) => [f, '\u0000not-loaded']),
	...TAG_NUMBER_FIELDS.map((f) => [f, NaN])
]) as TagData;

function intersectTags(tagsList: TagData[]): TagData {
	const result: TagData = {};

	for (const field of TAG_FIELDS) {
		const values = tagsList.map((t) => t[field] ?? '');
		const allSame = values.every((v) => v === values[0]);
		(result as any)[field] = allSame ? values[0] || undefined : KEEP_VALUE;
	}

	for (const field of TAG_NUMBER_FIELDS) {
		const values = tagsList.map((t) => t[field] ?? undefined);
		const allSame = values.every((v) => v === values[0]);
		(result as any)[field] = allSame ? values[0] : KEEP_VALUE;
	}

	return result;
}

// Generation counter — incremented on clearTags() to invalidate in-flight fetches
let fetchGeneration = 0;

// IDs with a tag fetch currently in flight
const tagsInFlight = new Set<string>();

// Fetch tags for a set of file IDs
export async function fetchTagsForFiles(ids: string[], force = false) {
	const $filesById = get(filesById);
	const $loaded = get(loadedTags);
	const gen = fetchGeneration;

	// Only fetch for files we don't already have or aren't already fetching (unless forced)
	const $unreadable = get(unreadableIds);
	const needed = force
		? ids
		: ids.filter((id) => !$loaded.has(id) && !tagsInFlight.has(id) && !$unreadable.has(id));
	if (needed.length === 0) return;

	// Build id -> relative_path map
	const paths: Record<string, string> = {};
	for (const id of needed) {
		const file = $filesById.get(id);
		if (file) paths[id] = file.relative_path;
	}

	if (Object.keys(paths).length === 0) return;

	for (const id of needed) tagsInFlight.add(id);
	try {
		const tags = await readTags(needed, paths);
		// Discard if directory changed while fetching
		if (gen !== fetchGeneration) return;
		unreadableIds.update((set) => {
			const next = new Set(set);
			for (const id of Object.keys(paths)) {
				if (id in tags) next.delete(id);
				else next.add(id);
			}
			return next;
		});
		loadedTags.update((map) => {
			const next = new Map(map);
			for (const [id, data] of Object.entries(tags)) {
				const prev = next.get(id);
				next.set(
					id,
					prev
						? {
								...data,
								bitrate: prev.bitrate,
								sample_rate: prev.sample_rate,
								channels: prev.channels,
								duration_secs: prev.duration_secs
							}
						: data
				);
			}
			return next;
		});
	} catch (err) {
		console.error('Failed to fetch tags:', err);
		if ((err as ApiError).status !== 401) toast.error('Failed to load tags.');
	} finally {
		if (gen === fetchGeneration) for (const id of needed) tagsInFlight.delete(id);
	}
}

// Debounced fetch for visible grid rows: fast tags first, then properties backfill
let visibleTagsTimer: ReturnType<typeof setTimeout> | null = null;
let pendingVisibleIds: string[] = [];

export function queueVisibleTagsFetch(ids: string[]) {
	pendingVisibleIds = [...new Set([...pendingVisibleIds, ...ids])];

	if (visibleTagsTimer) clearTimeout(visibleTagsTimer);
	visibleTagsTimer = setTimeout(() => {
		visibleTagsTimer = null;
		const batch = pendingVisibleIds;
		pendingVisibleIds = [];
		fetchTagsForFiles(batch).then(() => {
			queuePropertiesFetch(batch);
		});
	}, 150);
}

// Track which files have had properties loaded
const propertiesLoaded = new Set<string>();

// Fetch audio properties (duration, bitrate) for files that already have fast tags.
// Called as a background backfill after the grid is populated.
let propertiesTimer: ReturnType<typeof setTimeout> | null = null;
let pendingPropertyIds: string[] = [];

let propertiesRunning = false;
let propertiesBatch = new Set<string>();

async function processNextPropertiesBatch() {
	propertiesTimer = null;
	propertiesRunning = true;
	const batch = pendingPropertyIds.splice(0, 50);
	propertiesBatch = new Set(batch);
	if (batch.length > 0) await fetchPropertiesForFiles(batch);
	propertiesBatch = new Set();
	propertiesRunning = false;
	if (pendingPropertyIds.length > 0 && !propertiesTimer) {
		propertiesTimer = setTimeout(processNextPropertiesBatch, 50);
	}
}

export function queuePropertiesFetch(ids: string[]) {
	const needed = ids.filter((id) => !propertiesLoaded.has(id) && !propertiesBatch.has(id));
	if (needed.length === 0) return;
	pendingPropertyIds = [...new Set([...pendingPropertyIds, ...needed])];
	if (propertiesRunning) return;

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
		for (const id of Object.keys(paths)) propertiesLoaded.add(id);
		if (Object.keys(tags).length === 0) return;
		loadedTags.update((map) => {
			const next = new Map(map);
			for (const [id, data] of Object.entries(tags)) {
				const existing = next.get(id);
				// Merge: keep existing tag fields, add audio properties
				const { bitrate, sample_rate, channels, duration_secs } = data;
				next.set(id, existing ? { ...existing, bitrate, sample_rate, channels, duration_secs } : data);
			}
			return next;
		});
	} catch (err) {
		console.error('Failed to fetch properties:', err);
	}
}

// Set a pending edit for a field on all currently selected files
function editableSelection(): string[] {
	const $unreadable = get(unreadableIds);
	return Array.from(get(selectedIds)).filter((id) => !$unreadable.has(id));
}

export function setPendingEdit(field: string, value: string | number | null | undefined) {
	const $selected = editableSelection();
	if ($selected.length === 0) return;

	const $loaded = get(loadedTags);
	pendingEdits.update((map) => {
		const next = new Map(map);
		for (const id of $selected) {
			const existing = { ...(next.get(id) || {}) } as Record<string, unknown>;
			const loaded = $loaded.get(id) as Record<string, unknown> | undefined;
			if (loaded && (loaded[field] ?? null) === (value ?? null)) delete existing[field];
			else existing[field] = value;
			if (Object.keys(existing).length === 0) next.delete(id);
			else next.set(id, existing as TagEdits);
		}
		return next;
	});
}

export function clearPendingEdit(field: string) {
	const $selected = editableSelection();
	const $loaded = get(loadedTags);
	const values = $selected.map((id) => ($loaded.get(id) as any)?.[field] ?? '');
	const removeShared = values.length > 0 && values[0] !== '' && values.every((v) => v === values[0]);
	pendingEdits.update((map) => {
		const next = new Map(map);
		for (const id of get(selectedIds)) {
			const existing = next.get(id);
			if (!existing || !(field in existing)) continue;
			const rest = { ...existing };
			delete rest[field as keyof TagEdits];
			if (Object.keys(rest).length === 0) next.delete(id);
			else next.set(id, rest);
		}
		return next;
	});
	if (removeShared) setPendingEdit(field, null);
}

type SaveResult = { success: number; failed: number; failedIds: string[] };

let saveInFlight: Promise<SaveResult> | null = null;
let saveSnapshot: Map<string, TagEdits> | null = null;

// Save all pending edits to the server
export function saveAllEdits(): Promise<SaveResult> {
	const $pending = get(pendingEdits);
	if (saveInFlight && $pending === saveSnapshot) return saveInFlight;
	saveSnapshot = $pending;
	const run: Promise<SaveResult> = (saveInFlight ? saveInFlight.then(writeAllEdits) : writeAllEdits())
		.finally(() => {
			if (saveInFlight === run) saveInFlight = null;
		});
	saveInFlight = run;
	return run;
}

async function writeAllEdits(): Promise<SaveResult> {
	const $filesById = get(filesById);
	const orphaned = $filesById.size === 0
		? []
		: Array.from(get(pendingEdits).keys()).filter((id) => !$filesById.has(id));
	if (orphaned.length > 0) {
		pendingEdits.update((map) => {
			const next = new Map(map);
			for (const id of orphaned) next.delete(id);
			return next;
		});
		toast.warning(`Dropped unsaved edits for ${orphaned.length} file(s) no longer in this folder.`);
	}
	const $pending = get(pendingEdits);

	if ($pending.size === 0) return { success: 0, failed: 0, failedIds: [] };

	const changes = Array.from($pending.entries()).map(([id, edits]) => {
		const file = $filesById.get(id);
		return {
			id,
			path: file?.relative_path ?? '',
			tags: edits
		};
	}).filter((c) => c.path !== '');

	if (changes.length === 0) return { success: 0, failed: $pending.size, failedIds: Array.from($pending.keys()) };

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
export function clearTags(keepEdits = false) {
	fetchGeneration++; // invalidate any in-flight fetches
	loadedTags.set(new Map());
	unreadableIds.set(new Set());
	if (!keepEdits) pendingEdits.set(new Map());
	tagsInFlight.clear();
	propertiesLoaded.clear();
	propertiesBatch = new Set();
	pendingPropertyIds = [];
	if (propertiesTimer) {
		clearTimeout(propertiesTimer);
		propertiesTimer = null;
	}
	pendingVisibleIds = [];
	if (visibleTagsTimer) {
		clearTimeout(visibleTagsTimer);
		visibleTagsTimer = null;
	}
}
