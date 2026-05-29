import { apiFetch } from './client';

interface RenameFileEntry {
	id: string;
	path: string;
}

export interface RenamePreview {
	id: string;
	old_name: string;
	new_name: string;
	conflict: boolean;
}

export interface RenameResult {
	id: string;
	status: string;
	old_name: string;
	new_name: string;
	/** Relative path of the file after the rename (new location on success). */
	new_relative_path: string;
	error?: string;
}

export async function previewRenames(
	files: RenameFileEntry[],
	format: string
): Promise<RenamePreview[]> {
	const res = await apiFetch<{ previews: RenamePreview[] }>('/rename/preview', {
		method: 'POST',
		body: JSON.stringify({ files, format })
	});
	return res.previews;
}

export async function executeRenames(
	files: RenameFileEntry[],
	format: string
): Promise<RenameResult[]> {
	const res = await apiFetch<{ results: RenameResult[] }>('/rename/execute', {
		method: 'POST',
		body: JSON.stringify({ files, format })
	});
	return res.results;
}
