export function formatDuration(secs: number | null | undefined): string {
	if (secs == null) return '';
	const m = Math.floor(secs / 60);
	const s = Math.floor(secs % 60);
	return `${m}:${s.toString().padStart(2, '0')}`;
}

export function formatTotalDuration(secs: number): string {
	if (secs > 0 && secs < 60) {
		return `${Math.round(secs)}s`;
	}
	const h = Math.floor(secs / 3600);
	const m = Math.floor((secs % 3600) / 60);
	if (h > 0) return `${h}h ${m}m`;
	return `${m}m`;
}

export function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
	if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
	return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
