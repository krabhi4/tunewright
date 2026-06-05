import { apiFetch } from './client';

const BASE = '/api/v1';

export function getCoverArtUrl(relativePath: string, size: number = 250): string {
	const params = new URLSearchParams({
		path: relativePath,
		size: String(size)
	});
	return `${BASE}/coverart?${params}`;
}

export interface EmbedCoverArtResponse {
	status: string;
	embedded: number;
	errors: string[];
}

export async function embedCoverArtFromUrl(url: string, paths: string[]): Promise<EmbedCoverArtResponse> {
	const res = await apiFetch<EmbedCoverArtResponse>('/coverart/from-url', {
		method: 'POST',
		body: JSON.stringify({ url, paths })
	});
	if (res.embedded === 0 && res.errors && res.errors.length > 0) {
		throw new Error(`Failed to embed cover art: ${res.errors.join(', ')}`);
	}
	return res;
}

export async function uploadCoverArt(relativePath: string, imageData: Blob): Promise<void> {
	const form = new FormData();
	form.append('path', relativePath);
	form.append('image', imageData);

	// apiFetch detects the FormData body (skips the JSON header) and gives us
	// the shared 401-redirect + error handling.
	await apiFetch<void>('/coverart', { method: 'POST', body: form });
}
