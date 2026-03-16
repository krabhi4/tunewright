import { apiFetch } from './client';
import type { FileListResult, DirNode } from '$lib/types/audio';

export async function listFiles(
	path: string = '/',
	offset: number = 0,
	limit: number = 500
): Promise<FileListResult> {
	const params = new URLSearchParams({
		path,
		offset: String(offset),
		limit: String(limit)
	});
	return apiFetch<FileListResult>(`/files?${params}`);
}

export async function getDirTree(depth: number = 3): Promise<DirNode> {
	const params = new URLSearchParams({ depth: String(depth) });
	return apiFetch<DirNode>(`/files/tree?${params}`);
}
