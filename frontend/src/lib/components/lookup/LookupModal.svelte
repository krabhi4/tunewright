<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry } from '$lib/types/audio';
	import { searchMusicBrainz, getMusicBrainzRelease } from '$lib/api/lookup';
	import type { ReleaseSearchResult, ReleaseDetail, TrackInfo } from '$lib/api/lookup';
	import { setPendingEdit, pendingEdits, mergedTags, saveAllEdits } from '$lib/stores/tags';
	import { executeRenames } from '$lib/api/rename';
	import { selectedIds, files } from '$lib/stores/files';
	import { get } from 'svelte/store';
	import { selectedTags } from '$lib/stores/tags';
	import { formatDuration } from '$lib/utils/format';

	interface Props {
		open: boolean;
		onClose: () => void;
	}

	let { open, onClose }: Props = $props();

	// Step: 'search' | 'match'
	let step = $state<'search' | 'match'>('search');

	// Search state
	let searchQuery = $state('');
	let searchResults = $state<ReleaseSearchResult[]>([]);
	let selectedRelease = $state<ReleaseDetail | null>(null);
	let searching = $state(false);
	let loadingRelease = $state(false);

	// Matching state: tracks on left, files on right, linked by index
	let matchedFiles = $state<(FileEntry | null)[]>([]);
	let unmatchedFiles = $state<FileEntry[]>([]);
	let dragSourceIdx = $state<number | null>(null);
	let dragFromUnmatched = $state<number | null>(null);
	let renameFiles = $state(false);
	let applying = $state(false);

	// Auto-fill search from selected tags
	$effect(() => {
		if (open) {
			const tags = $selectedTags;
			if (tags) {
				const parts = [tags.artist, tags.album].filter(
					(v) => v && v !== '< keep >'
				);
				searchQuery = parts.join(' ');
			}
			selectedRelease = null;
			searchResults = [];
			step = 'search';
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
			if (selectedRelease) {
				initMatchingStep();
			}
		} catch (err) {
			console.error('Failed to load release:', err);
		} finally {
			loadingRelease = false;
		}
	}

	function initMatchingStep() {
		if (!selectedRelease) return;

		const currentSelectedIds = get(selectedIds);
		const currentFiles = get(files);
		const selectedFilesList = currentFiles
			.filter((f) => currentSelectedIds.has(f.id))
			.sort((a, b) => a.filename.localeCompare(b.filename));

		const tracks = selectedRelease.tracks;

		// Auto-match: pair files to tracks positionally
		matchedFiles = tracks.map((_, i) =>
			i < selectedFilesList.length ? selectedFilesList[i] : null
		);

		// Any extra files beyond the track count go to unmatched
		unmatchedFiles =
			selectedFilesList.length > tracks.length
				? selectedFilesList.slice(tracks.length)
				: [];

		step = 'match';
	}

	// Drag from matched slot
	function onMatchedDragStart(idx: number) {
		dragSourceIdx = idx;
		dragFromUnmatched = null;
	}

	// Drag from unmatched pool
	function onUnmatchedDragStart(idx: number) {
		dragFromUnmatched = idx;
		dragSourceIdx = null;
	}

	function onMatchedDrop(targetIdx: number, e: DragEvent) {
		e.preventDefault();

		if (dragSourceIdx !== null && dragSourceIdx !== targetIdx) {
			// Swap two matched slots
			const temp = matchedFiles[targetIdx];
			matchedFiles[targetIdx] = matchedFiles[dragSourceIdx];
			matchedFiles[dragSourceIdx] = temp;
			matchedFiles = [...matchedFiles];
		} else if (dragFromUnmatched !== null) {
			// Move from unmatched into a matched slot
			const file = unmatchedFiles[dragFromUnmatched];
			// If target slot already has a file, send it to unmatched
			const displaced = matchedFiles[targetIdx];
			matchedFiles[targetIdx] = file;
			matchedFiles = [...matchedFiles];
			unmatchedFiles = unmatchedFiles.filter((_, i) => i !== dragFromUnmatched);
			if (displaced) unmatchedFiles = [...unmatchedFiles, displaced];
		}

		dragSourceIdx = null;
		dragFromUnmatched = null;
	}

	function onUnmatchedDrop(e: DragEvent) {
		e.preventDefault();
		if (dragSourceIdx !== null) {
			// Move from matched slot to unmatched
			const file = matchedFiles[dragSourceIdx];
			if (file) {
				matchedFiles[dragSourceIdx] = null;
				matchedFiles = [...matchedFiles];
				unmatchedFiles = [...unmatchedFiles, file];
			}
		}
		dragSourceIdx = null;
		dragFromUnmatched = null;
	}

	// Remove a file from a matched slot (send to unmatched)
	function unmatchFile(idx: number) {
		const file = matchedFiles[idx];
		if (file) {
			matchedFiles[idx] = null;
			matchedFiles = [...matchedFiles];
			unmatchedFiles = [...unmatchedFiles, file];
		}
	}

	// Auto-match by track number from existing tags
	function autoMatchByTrackNumber() {
		if (!selectedRelease) return;
		const currentTags = get(mergedTags);

		const allFiles = [
			...matchedFiles.filter((f): f is FileEntry => f !== null),
			...unmatchedFiles
		];

		const newMatched: (FileEntry | null)[] = selectedRelease.tracks.map(() => null);
		const used = new Set<string>();

		for (const file of allFiles) {
			const tags = currentTags.get(file.id);
			const trackNum = tags?.track_number;
			if (trackNum != null) {
				const idx = selectedRelease.tracks.findIndex((t) => t.position === trackNum);
				if (idx !== -1 && newMatched[idx] === null) {
					newMatched[idx] = file;
					used.add(file.id);
				}
			}
		}

		matchedFiles = newMatched;
		unmatchedFiles = allFiles.filter((f) => !used.has(f.id));
	}

	async function applyMatches() {
		if (!selectedRelease) return;
		applying = true;
		try {
			const tracks = selectedRelease.tracks;
			const filesToRename: { id: string; path: string; track: TrackInfo }[] = [];

			for (let i = 0; i < tracks.length; i++) {
				const file = matchedFiles[i];
				if (!file) continue;

				const track = tracks[i];

				pendingEdits.update((map) => {
					const next = new Map(map);
					const existing = next.get(file.id) || {};
					next.set(file.id, {
						...existing,
						title: track.title,
						track_number: track.position,
						album: selectedRelease!.title,
						album_artist: selectedRelease!.artist,
						...(selectedRelease!.year ? { year: selectedRelease!.year } : {}),
						...(selectedRelease!.genre ? { genre: selectedRelease!.genre } : {}),
						...(track.artist ? { artist: track.artist } : { artist: selectedRelease!.artist })
					});
					return next;
				});

				if (renameFiles) {
					filesToRename.push({ id: file.id, path: file.relative_path, track });
				}
			}

			if (renameFiles && filesToRename.length > 0) {
				await saveAllEdits();
				try {
					await executeRenames(
						filesToRename.map((f) => ({ id: f.id, path: f.path })),
						'%track% - %title%'
					);
				} catch (err) {
					console.error('Rename failed:', err);
				}
			}

			onClose();
		} catch (err) {
			console.error('Apply failed:', err);
		} finally {
			applying = false;
		}
	}

	function handleSearchKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleSearch();
	}

	let matchedCount = $derived(matchedFiles.filter((f) => f !== null).length);
</script>

<Modal title="MusicBrainz Lookup" {open} {onClose}>
	{#if step === 'search'}
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
						<div class="release-tracks-count">{selectedRelease.tracks.length} tracks</div>
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
					<button class="btn-apply" onclick={initMatchingStep}>
						Match to Files
					</button>
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

	{:else if step === 'match' && selectedRelease}
		<div class="match-header">
			<div class="match-info">
				<strong>{selectedRelease.title}</strong> by {selectedRelease.artist}
				{#if selectedRelease.year} ({selectedRelease.year}){/if}
			</div>
			<div class="match-actions">
				<button class="btn-small" onclick={autoMatchByTrackNumber}>Auto-match by #</button>
			</div>
		</div>

		<div class="match-grid">
			<div class="match-col-header">
				<span class="col-label">Lookup Track</span>
				<span class="col-label">Your File</span>
			</div>

			{#each selectedRelease.tracks as track, i}
				<div class="match-row">
					<div class="match-track">
						<span class="track-num mono">{track.position}</span>
						<span class="track-title">{track.title}</span>
						{#if track.duration_secs}
							<span class="track-dur mono">{formatDuration(track.duration_secs)}</span>
						{/if}
					</div>
					<div class="match-arrow">{matchedFiles[i] ? '\u2194' : ''}</div>
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="match-file"
						class:empty={!matchedFiles[i]}
						class:drag-over={false}
						ondragover={(e) => e.preventDefault()}
						ondrop={(e) => onMatchedDrop(i, e)}
					>
						{#if matchedFiles[i]}
							{@const file = matchedFiles[i]}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="file-chip"
								draggable="true"
								ondragstart={() => onMatchedDragStart(i)}
							>
								<span class="file-chip-name">{file.filename}</span>
								<button class="file-chip-remove" onclick={() => unmatchFile(i)}>&times;</button>
							</div>
						{:else}
							<span class="drop-hint">Drop file here</span>
						{/if}
					</div>
				</div>
			{/each}
		</div>

		{#if unmatchedFiles.length > 0}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="unmatched-pool"
				ondragover={(e) => e.preventDefault()}
				ondrop={onUnmatchedDrop}
			>
				<div class="unmatched-label">Unmatched files ({unmatchedFiles.length})</div>
				<div class="unmatched-list">
					{#each unmatchedFiles as file, i}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="file-chip"
							draggable="true"
							ondragstart={() => onUnmatchedDragStart(i)}
						>
							<span class="file-chip-name">{file.filename}</span>
						</div>
					{/each}
				</div>
			</div>
		{/if}

		<div class="apply-bar">
			<button class="btn-back" onclick={() => (step = 'search')}>Back</button>
			<label class="rename-check">
				<input type="checkbox" bind:checked={renameFiles} />
				<span>Rename files to match</span>
			</label>
			<span class="match-count">{matchedCount}/{selectedRelease.tracks.length} matched</span>
			<button class="btn-apply" onclick={applyMatches} disabled={matchedCount === 0 || applying}>
				{applying ? 'Applying...' : `Apply ${matchedCount} Matches`}
			</button>
		</div>
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

	.release-tracks-count {
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
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
		align-items: center;
		gap: 8px;
		margin-top: 10px;
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

	.btn-back:hover {
		background: var(--bg-hover);
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

	.btn-apply:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.btn-apply:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.status-msg {
		color: var(--text-muted);
		font-size: 12px;
		padding: 20px;
		text-align: center;
	}

	/* Matching step styles */
	.match-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 10px;
		padding-bottom: 8px;
		border-bottom: 1px solid var(--border-subtle);
	}

	.match-info {
		font-size: 12px;
		color: var(--text-secondary);
	}

	.match-info strong {
		color: var(--text-primary);
	}

	.match-actions {
		display: flex;
		gap: 4px;
	}

	.btn-small {
		background: var(--bg-elevated);
		border: 1px solid var(--border-subtle);
		color: var(--text-secondary);
		padding: 3px 8px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 10.5px;
	}

	.btn-small:hover {
		background: var(--accent-subtle);
		color: var(--accent);
	}

	.match-grid {
		max-height: 320px;
		overflow-y: auto;
		margin-bottom: 8px;
	}

	.match-col-header {
		display: grid;
		grid-template-columns: 1fr 24px 1fr;
		gap: 4px;
		padding: 4px 0;
		border-bottom: 1px solid var(--border);
	}

	.col-label {
		font-size: 10px;
		font-weight: 600;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}

	.match-row {
		display: grid;
		grid-template-columns: 1fr 24px 1fr;
		gap: 4px;
		align-items: center;
		padding: 3px 0;
		border-bottom: 1px solid var(--grid-border);
	}

	.match-track {
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 11.5px;
		min-width: 0;
	}

	.match-arrow {
		text-align: center;
		color: var(--text-muted);
		font-size: 12px;
	}

	.match-file {
		min-height: 26px;
		display: flex;
		align-items: center;
		border-radius: var(--radius-sm);
		transition: background 0.1s;
	}

	.match-file.empty {
		border: 1px dashed var(--border-subtle);
		justify-content: center;
	}

	.file-chip {
		display: flex;
		align-items: center;
		gap: 4px;
		background: var(--bg-elevated);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		padding: 2px 6px;
		cursor: grab;
		max-width: 100%;
		min-width: 0;
	}

	.file-chip:active {
		cursor: grabbing;
		opacity: 0.7;
	}

	.file-chip-name {
		font-size: 11px;
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-primary);
	}

	.file-chip-remove {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 14px;
		line-height: 1;
		padding: 0 2px;
		flex-shrink: 0;
	}

	.file-chip-remove:hover {
		color: var(--error);
	}

	.drop-hint {
		font-size: 10px;
		color: var(--text-muted);
	}

	.unmatched-pool {
		border: 1px dashed var(--border-subtle);
		border-radius: var(--radius-sm);
		padding: 6px 8px;
		margin-bottom: 4px;
	}

	.unmatched-label {
		font-size: 10px;
		font-weight: 600;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.3px;
		margin-bottom: 4px;
	}

	.unmatched-list {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.rename-check {
		display: flex;
		align-items: center;
		gap: 4px;
		font-size: 11px;
		color: var(--text-secondary);
		cursor: pointer;
		margin-right: auto;
	}

	.rename-check input {
		accent-color: var(--accent);
		cursor: pointer;
	}

	.match-count {
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--text-secondary);
	}
</style>
