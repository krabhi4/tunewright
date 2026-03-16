<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry } from '$lib/types/audio';
	import { searchMusicBrainz, getMusicBrainzRelease } from '$lib/api/lookup';
	import type { ReleaseSearchResult, ReleaseDetail } from '$lib/api/lookup';
	import { selectedTags, setPendingEdit, pendingEdits } from '$lib/stores/tags';
	import { selectedIds, files } from '$lib/stores/files';
	import { get } from 'svelte/store';
	import { formatDuration } from '$lib/utils/format';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	let searchQuery = $state('');
	let searchResults = $state<ReleaseSearchResult[]>([]);
	let selectedRelease = $state<ReleaseDetail | null>(null);
	let searching = $state(false);
	let loadingRelease = $state(false);

	// Auto-fill search from selected tags
	$effect(() => {
		if (open) {
			const tags = $selectedTags;
			if (tags) {
				const parts = [tags.artist, tags.album].filter(Boolean);
				searchQuery = parts.join(' ');
			}
			selectedRelease = null;
			searchResults = [];
		}
	});

	async function handleSearch() {
		if (!searchQuery.trim()) return;
		searching = true;
		searchResults = [];
		selectedRelease = null;
		try {
			searchResults = await searchMusicBrainz(searchQuery);
		} catch (err) {
			console.error('Search failed:', err);
		} finally {
			searching = false;
		}
	}

	async function selectRelease(result: ReleaseSearchResult) {
		loadingRelease = true;
		try {
			selectedRelease = await getMusicBrainzRelease(result.id);
		} catch (err) {
			console.error('Failed to load release:', err);
		} finally {
			loadingRelease = false;
		}
	}

	function applyToSelected() {
		if (!selectedRelease) return;
		const currentSelectedIds = get(selectedIds);
		const currentFiles = get(files);
		const selectedFilesList = currentFiles.filter((f) => currentSelectedIds.has(f.id));

		// Apply album-level tags to all selected files
		for (const _file of selectedFilesList) {
			if (selectedRelease.artist) setPendingEdit('artist', selectedRelease.artist);
			if (selectedRelease.title) setPendingEdit('album', selectedRelease.title);
			if (selectedRelease.year) setPendingEdit('year', selectedRelease.year);
			if (selectedRelease.genre) setPendingEdit('genre', selectedRelease.genre);
		}

		// Match tracks by position if possible
		const tracks = selectedRelease.tracks;
		const sortedFiles = [...selectedFilesList].sort((a, b) =>
			a.filename.localeCompare(b.filename)
		);

		// Simple positional matching
		for (let i = 0; i < Math.min(sortedFiles.length, tracks.length); i++) {
			const track = tracks[i];
			const fileId = sortedFiles[i].id;

			// Set track-specific edits directly on pendingEdits
			pendingEdits.update((map) => {
				const next = new Map(map);
				const existing = next.get(fileId) || {};
				next.set(fileId, {
					...existing,
					title: track.title,
					track_number: track.position,
					...(track.artist ? { artist: track.artist } : {})
				});
				return next;
			});
		}

		onClose();
	}

	function handleSearchKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleSearch();
	}
</script>

<Modal title="MusicBrainz Lookup" {open} {onClose}>
	<div class="search-bar">
		<input
			class="search-input"
			type="text"
			bind:value={searchQuery}
			onkeydown={handleSearchKeydown}
			placeholder="Search artist, album..."
		/>
		<button class="search-btn" onclick={handleSearch} disabled={searching}>
			{searching ? 'Searching...' : 'Search'}
		</button>
	</div>

	{#if selectedRelease}
		<div class="release-detail">
			<div class="release-header">
				{#if selectedRelease.cover_art_url}
					<img
						src={selectedRelease.cover_art_url}
						alt="Cover"
						class="release-cover"
						onerror={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
					/>
				{/if}
				<div class="release-info">
					<div class="release-title">{selectedRelease.title}</div>
					<div class="release-artist">{selectedRelease.artist}</div>
					{#if selectedRelease.year}
						<div class="release-year">{selectedRelease.year}</div>
					{/if}
				</div>
			</div>
			<div class="track-list">
				{#each selectedRelease.tracks as track}
					<div class="track-row">
						<span class="track-num mono">{track.position}</span>
						<span class="track-title">{track.title}</span>
						{#if track.duration_secs}
							<span class="track-dur mono">{formatDuration(track.duration_secs)}</span>
						{/if}
					</div>
				{/each}
			</div>
			<div class="apply-bar">
				<button class="btn-back" onclick={() => (selectedRelease = null)}>Back</button>
				<button class="btn-apply" onclick={applyToSelected}>Apply to Selected</button>
			</div>
		</div>
	{:else if searchResults.length > 0}
		<div class="results-list">
			{#each searchResults as result (result.id)}
				<button class="result-row" onclick={() => selectRelease(result)}>
					<div class="result-info">
						<span class="result-title">{result.title}</span>
						<span class="result-artist">{result.artist}</span>
					</div>
					<div class="result-meta">
						{#if result.year}<span class="result-year">{result.year}</span>{/if}
						{#if result.track_count}<span class="result-tracks">{result.track_count} tracks</span>{/if}
					</div>
				</button>
			{/each}
		</div>
	{:else if searching}
		<div class="status-msg">Searching MusicBrainz...</div>
	{:else if loadingRelease}
		<div class="status-msg">Loading release details...</div>
	{/if}
</Modal>

<style>
	.search-bar {
		display: flex;
		gap: 6px;
		margin-bottom: 12px;
	}

	.search-input {
		flex: 1;
		background: var(--bg-base);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-family: var(--font-ui);
		font-size: 12px;
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		outline: none;
	}

	.search-input:focus {
		border-color: var(--accent);
	}

	.search-btn {
		background: var(--accent);
		border: none;
		color: white;
		padding: 6px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
		white-space: nowrap;
	}

	.search-btn:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.search-btn:disabled {
		opacity: 0.5;
	}

	.results-list {
		max-height: 350px;
		overflow-y: auto;
	}

	.result-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 8px;
		background: none;
		border: none;
		border-bottom: 1px solid var(--grid-border);
		cursor: pointer;
		text-align: left;
		color: var(--text-primary);
		font-family: var(--font-ui);
	}

	.result-row:hover {
		background: var(--bg-hover);
	}

	.result-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.result-title {
		font-size: 12.5px;
	}

	.result-artist {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.result-meta {
		display: flex;
		gap: 8px;
		font-size: 11px;
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.release-detail {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		padding: 10px;
	}

	.release-header {
		display: flex;
		gap: 12px;
		margin-bottom: 10px;
	}

	.release-cover {
		width: 80px;
		height: 80px;
		object-fit: cover;
		border-radius: var(--radius-sm);
		flex-shrink: 0;
	}

	.release-info {
		flex: 1;
	}

	.release-title {
		font-size: 14px;
		font-weight: 600;
	}

	.release-artist {
		font-size: 12px;
		color: var(--text-secondary);
		margin-top: 2px;
	}

	.release-year {
		font-size: 11px;
		color: var(--text-muted);
		margin-top: 2px;
	}

	.track-list {
		max-height: 200px;
		overflow-y: auto;
		margin-bottom: 10px;
	}

	.track-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 3px 0;
		font-size: 11.5px;
		border-bottom: 1px solid var(--grid-border);
	}

	.track-num {
		width: 24px;
		color: var(--text-muted);
		text-align: right;
		flex-shrink: 0;
	}

	.track-title {
		flex: 1;
	}

	.track-dur {
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.mono {
		font-family: var(--font-mono);
		font-size: 11px;
	}

	.apply-bar {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	.btn-back {
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		padding: 5px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
	}

	.btn-apply {
		background: var(--accent);
		border: none;
		color: white;
		padding: 5px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
	}

	.btn-apply:hover {
		background: var(--accent-hover);
	}

	.status-msg {
		color: var(--text-muted);
		font-size: 12px;
		padding: 20px;
		text-align: center;
	}
</style>
