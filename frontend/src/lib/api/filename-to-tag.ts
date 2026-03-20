import { apiFetch } from './client';
import type { TagData } from '$lib/types/audio';

interface FileEntry {
	id: string;
	path: string;
}

export interface FilenameTagPreview {
	id: string;
	filename: string;
	matched: boolean;
	tags?: Partial<TagData>;
}

export async function previewFilenameToTag(
	files: FileEntry[],
	pattern: string
): Promise<FilenameTagPreview[]> {
	const res = await apiFetch<{ previews: FilenameTagPreview[] }>(
		'/filename-to-tag/preview',
		{
			method: 'POST',
			body: JSON.stringify({ files, pattern })
		}
	);
	return res.previews;
}
