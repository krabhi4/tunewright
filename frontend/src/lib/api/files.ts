import { apiFetch } from './client';
import type { FileListResult } from '$lib/types/audio';

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
