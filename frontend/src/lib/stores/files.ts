import { writable, derived, get } from 'svelte/store';
import type { FileEntry } from '$lib/types/audio';
import { listFiles } from '$lib/api/files';

export const currentPath = writable('/');
export const files = writable<FileEntry[]>([]);
export const totalCount = writable(0);
export const directories = writable<string[]>([]);
export const selectedIds = writable<Set<string>>(new Set());
export const focusedId = writable<string | null>(null);
export const loading = writable(false);

export const selectedFiles = derived([files, selectedIds], ([$files, $selectedIds]) =>
	$files.filter((f) => $selectedIds.has(f.id))
);

// Index of files by id for O(1) lookups (avoids repeated linear scans of the list)
export const filesById = derived(files, ($files) => {
	const map = new Map<string, FileEntry>();
	for (const f of $files) map.set(f.id, f);
	return map;
});

export const selectedCount = derived(selectedIds, ($ids) => $ids.size);

let loadGeneration = 0;

export async function loadDirectory(path: string) {
	loadGeneration++;
	const gen = loadGeneration;

	loading.set(true);
	currentPath.set(path);
	selectedIds.set(new Set());

	try {
		const result = await listFiles(path, 0, 5000);
		if (gen !== loadGeneration) return;

		files.set(result.files);
		totalCount.set(result.total);
		directories.set(result.directories);
	} catch (err) {
		if (gen !== loadGeneration) return;
		console.error('Failed to load directory:', err);
		files.set([]);
		totalCount.set(0);
		directories.set([]);
	} finally {
		if (gen === loadGeneration) {
			loading.set(false);
		}
	}
}

export function toggleSelection(id: string, multi: boolean = false) {
	selectedIds.update((ids) => {
		const next = multi ? new Set(ids) : new Set<string>();
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		return next;
	});
}

export function selectAll() {
	const $files = get(files);
	selectedIds.set(new Set($files.map((f) => f.id)));
}

export function selectNone() {
	selectedIds.set(new Set());
}

export function selectRange(fromId: string, toId: string, customFiles?: FileEntry[]) {
	const $files = customFiles || get(files);
	const fromIdx = $files.findIndex((f) => f.id === fromId);
	const toIdx = $files.findIndex((f) => f.id === toId);
	if (fromIdx === -1 || toIdx === -1) return;
	const start = Math.min(fromIdx, toIdx);
	const end = Math.max(fromIdx, toIdx);
	const rangeIds = $files.slice(start, end + 1).map((f) => f.id);
	selectedIds.update((ids) => {
		const next = new Set(ids);
		for (const id of rangeIds) next.add(id);
		return next;
	});
}
