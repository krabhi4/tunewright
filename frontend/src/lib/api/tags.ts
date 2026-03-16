import { apiFetch } from './client';
import type { TagData } from '$lib/types/audio';

interface ReadTagsResponse {
	tags: Record<string, TagData>;
}

export async function readTags(
	ids: string[],
	paths: Record<string, string>
): Promise<Record<string, TagData>> {
	const res = await apiFetch<ReadTagsResponse>('/tags/read', {
		method: 'POST',
		body: JSON.stringify({ ids, paths })
	});
	return res.tags;
}

interface WriteEntry {
	id: string;
	path: string;
	tags: Partial<TagData>;
}

interface WriteResult {
	id: string;
	status: string;
	error?: string;
}

interface WriteTagsResponse {
	results: WriteResult[];
}

export async function writeTags(changes: WriteEntry[]): Promise<WriteResult[]> {
	const res = await apiFetch<WriteTagsResponse>('/tags/write', {
		method: 'POST',
		body: JSON.stringify({ changes })
	});
	return res.results;
}
