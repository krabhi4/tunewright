import { apiFetch } from './client';

const BASE = '/api/v1';

export function getCoverArtUrl(relativePath: string, size: number = 250): string {
	const params = new URLSearchParams({
		path: relativePath,
		size: String(size)
	});
	return `${BASE}/coverart?${params}`;
}

export async function embedCoverArtFromUrl(url: string, paths: string[]): Promise<void> {
	await apiFetch<void>('/coverart/from-url', {
		method: 'POST',
		body: JSON.stringify({ url, paths })
	});
}

export async function uploadCoverArt(relativePath: string, imageData: Blob): Promise<void> {
	const form = new FormData();
	form.append('path', relativePath);
	form.append('image', imageData);

	// apiFetch detects the FormData body (skips the JSON header) and gives us
	// the shared 401-redirect + error handling.
	await apiFetch<void>('/coverart', { method: 'POST', body: form });
}
