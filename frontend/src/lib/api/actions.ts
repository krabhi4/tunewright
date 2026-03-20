import { apiFetch } from './client';

export interface Action {
	type: string;
	field?: string;
	mode?: string;
	search?: string;
	replace?: string;
	regex?: boolean;
	format?: string;
	value?: string;
	fields?: string[];
	start?: number;
	padding?: number;
	source?: string;
	separator?: string;
	part?: number;
	target?: string;
	sources?: string[];
}

interface FileEntry {
	id: string;
	path: string;
}

interface WriteResult {
	id: string;
	status: string;
	error?: string;
}

interface FieldChange {
	field: string;
	old_value: string;
	new_value: string;
}

export interface ActionPreview {
	id: string;
	filename: string;
	changes: FieldChange[];
}

export async function previewActions(
	files: FileEntry[],
	actions: Action[]
): Promise<ActionPreview[]> {
	const res = await apiFetch<{ previews: ActionPreview[] }>('/actions/preview', {
		method: 'POST',
		body: JSON.stringify({ files, actions })
	});
	return res.previews;
}

export async function executeActions(
	files: FileEntry[],
	actions: Action[]
): Promise<WriteResult[]> {
	const res = await apiFetch<{ results: WriteResult[] }>('/actions/execute', {
		method: 'POST',
		body: JSON.stringify({ files, actions })
	});
	return res.results;
}
