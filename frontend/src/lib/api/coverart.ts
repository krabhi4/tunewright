const BASE = '/api/v1';

export function getCoverArtUrl(relativePath: string, size: number = 250): string {
	const params = new URLSearchParams({
		path: relativePath,
		size: String(size)
	});
	return `${BASE}/coverart?${params}`;
}
