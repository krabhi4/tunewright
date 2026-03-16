<script lang="ts">
	import { selectedCount, selectedFiles } from '$lib/stores/files';
	import { selectedTags, KEEP_VALUE, setPendingEdit, mergedTags } from '$lib/stores/tags';
	import { getCoverArtUrl } from '$lib/api/coverart';

	let coverArtError = $state(false);

	// Get cover art URL for first selected file (check tags for has_cover)
	let coverArtUrl = $derived.by(() => {
		const files = $selectedFiles;
		if (files.length === 0) return null;
		const first = files[0];
		const tags = $mergedTags.get(first.id);
		if (!tags?.has_cover && !first.has_cover) return null;
		coverArtError = false;
		return getCoverArtUrl(first.relative_path, 250);
	});

	const fields = [
		{ key: 'title', label: 'Title' },
		{ key: 'artist', label: 'Artist' },
		{ key: 'album', label: 'Album' },
		{ key: 'album_artist', label: 'Album Artist' },
		{ key: 'year', label: 'Year' },
		{ key: 'track_number', label: 'Track' },
		{ key: 'genre', label: 'Genre' },
		{ key: 'composer', label: 'Composer' },
		{ key: 'comment', label: 'Comment' }
	] as const;

	function fieldValue(key: string): string {
		const tags = $selectedTags;
		if (!tags) return '';
		const val = (tags as any)[key];
		if (val === KEEP_VALUE) return KEEP_VALUE;
		if (val == null) return '';
		return String(val);
	}

	function isKeep(key: string): boolean {
		return fieldValue(key) === KEEP_VALUE;
	}

	function handleInput(key: string, e: Event) {
		const target = e.target as HTMLInputElement;
		const val = target.value;
		if (key === 'year' || key === 'track_number' || key === 'track_total' || key === 'disc_number' || key === 'disc_total') {
			const num = parseInt(val, 10);
			setPendingEdit(key, isNaN(num) ? undefined : num);
		} else {
			setPendingEdit(key, val);
		}
	}
</script>

<div class="tag-panel">
	<div class="panel-header">
		<span class="panel-label">Tag Panel</span>
		{#if $selectedCount > 0}
			<span class="panel-count">{$selectedCount}</span>
		{/if}
	</div>

	<div class="panel-body">
		{#if $selectedCount === 0}
			<div class="panel-empty">
				<span class="empty-text">Select files to edit tags</span>
			</div>
		{:else}
			{#each fields as field (field.key)}
				<div class="field-group">
					<!-- svelte-ignore a11y_label_has_associated_control -->
					<label class="field-label">{field.label}</label>
					<input
						class="field-input"
						class:keep={isKeep(field.key)}
						type="text"
						value={isKeep(field.key) ? '' : fieldValue(field.key)}
						placeholder={isKeep(field.key) ? KEEP_VALUE : '—'}
						onchange={(e) => handleInput(field.key, e)}
					/>
				</div>
			{/each}
		{/if}
	</div>

	<div class="cover-area">
		{#if coverArtUrl && !coverArtError}
			<img
				src={coverArtUrl}
				alt="Cover art"
				class="cover-img"
				onerror={() => (coverArtError = true)}
			/>
		{:else}
			<div class="cover-placeholder">
				<span class="cover-text">No Cover</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.tag-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--bg-surface);
		border-right: 1px solid var(--border);
		overflow: hidden;
	}

	.panel-header {
		padding: 8px 12px 6px;
		display: flex;
		align-items: center;
		gap: 6px;
		border-bottom: 1px solid var(--border-subtle);
		flex-shrink: 0;
	}

	.panel-label {
		font-size: 11px;
		font-weight: 600;
		color: var(--text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.4px;
	}

	.panel-count {
		font-size: 10px;
		font-family: var(--font-mono);
		color: var(--accent);
		background: var(--accent-subtle);
		padding: 1px 5px;
		border-radius: 8px;
	}

	.panel-body {
		flex: 1;
		overflow-y: auto;
		padding: 8px 12px;
	}

	.panel-empty {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 120px;
	}

	.empty-text {
		color: var(--text-muted);
		font-size: 12px;
	}

	.field-group {
		margin-bottom: 6px;
	}

	.field-label {
		display: block;
		font-size: 10.5px;
		color: var(--text-muted);
		margin-bottom: 2px;
		letter-spacing: 0.2px;
	}

	.field-input {
		width: 100%;
		background: transparent;
		border: 1px solid transparent;
		border-bottom-color: var(--border-subtle);
		color: var(--text-primary);
		font-family: var(--font-ui);
		font-size: 12.5px;
		padding: 3px 0;
		outline: none;
		transition: border-color 0.15s;
	}

	.field-input:focus {
		border-color: var(--accent);
	}

	.field-input.keep {
		color: var(--text-placeholder);
		font-style: italic;
	}

	.field-input.keep::placeholder {
		color: var(--text-placeholder);
		font-style: italic;
	}

	.cover-area {
		padding: 10px 12px 12px;
		flex-shrink: 0;
		border-top: 1px solid var(--border-subtle);
	}

	.cover-placeholder {
		aspect-ratio: 1;
		background: var(--bg-base);
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.3);
	}

	.cover-img {
		width: 100%;
		aspect-ratio: 1;
		object-fit: cover;
		border-radius: var(--radius-sm);
		box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.3);
	}

	.cover-text {
		color: var(--text-muted);
		font-size: 11px;
	}
</style>
