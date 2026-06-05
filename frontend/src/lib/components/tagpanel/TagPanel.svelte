<script lang="ts">
	import { selectedCount, selectedFiles, selectedIds } from '$lib/stores/files';
	import { selectedTags, KEEP_VALUE, setPendingEdit, mergedTags, pendingEdits } from '$lib/stores/tags';
	import { getCoverArtUrl, uploadCoverArt } from '$lib/api/coverart';
	import { coverArtVersion, bumpCoverArt } from '$lib/stores/ui';

	let coverArtError = $state(false);
	let dragOver = $state(false);
	let uploading = $state(false);
	let fileInput = $state<HTMLInputElement>();

	async function handleImageUpload(blob: Blob) {
		const files = $selectedFiles;
		if (files.length === 0) return;
		uploading = true;
		try {
			for (const file of files) {
				await uploadCoverArt(file.relative_path, blob);
			}
			coverArtError = false;
			// Force cover art refresh across components by bumping the global version
			bumpCoverArt();
		} catch (err) {
			console.error('Failed to upload cover art:', err);
		} finally {
			uploading = false;
		}
	}

	function onFileSelected(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (file && file.type.startsWith('image/')) handleImageUpload(file);
		input.value = '';
	}

	function onDrop(e: DragEvent) {
		e.preventDefault();
		dragOver = false;
		const file = e.dataTransfer?.files?.[0];
		if (file && file.type.startsWith('image/')) {
			handleImageUpload(file);
		}
	}

	function onPaste(e: ClipboardEvent) {
		if ($selectedFiles.length === 0) return;
		const items = e.clipboardData?.items;
		if (!items) return;
		for (const item of items) {
			if (item.type.startsWith('image/')) {
				const blob = item.getAsFile();
				if (blob) {
					e.preventDefault();
					handleImageUpload(blob);
					return;
				}
			}
		}
	}

	// Get cover art URL for first selected file (check tags for has_cover).
	// Once any cover art has changed this session ($coverArtVersion > 0) we
	// optimistically try to load, since has_cover from the listing can be stale.
	let coverArtUrl = $derived.by(() => {
		const files = $selectedFiles;
		const version = $coverArtVersion;
		if (files.length === 0) return null;
		const first = files[0];
		const tags = $mergedTags.get(first.id);
		if (!tags?.has_cover && !first.has_cover && version === 0) return null;
		return getCoverArtUrl(first.relative_path, 250) + `&_=${version}`;
	});

	$effect(() => {
		const _url = coverArtUrl;
		coverArtError = false;
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

	// Precompute each field's display value / keep / edited state once per render.
	// Previously isEdited() looped over every selected id for every field, and
	// value/keep were recomputed several times per field in the template.
	let fieldStates = $derived.by(() => {
		const tags = $selectedTags;
		const edits = $pendingEdits;
		const editedKeys = new Set<string>();
		for (const id of $selectedIds) {
			const fileEdits = edits.get(id);
			if (fileEdits) {
				for (const k of Object.keys(fileEdits)) editedKeys.add(k);
			}
		}
		const states: Record<string, { value: string; keep: boolean; edited: boolean }> = {};
		for (const { key } of fields) {
			const val = tags ? (tags as any)[key] : undefined;
			const keep = val === KEEP_VALUE;
			states[key] = {
				value: keep ? KEEP_VALUE : val == null ? '' : String(val),
				keep,
				edited: editedKeys.has(key)
			};
		}
		return states;
	});

	function handleInput(key: string, e: Event) {
		const target = e.target as HTMLInputElement;
		const val = target.value;
		if (key === 'year' || key === 'track_number' || key === 'track_total' || key === 'disc_number' || key === 'disc_total') {
			let num = parseInt(val, 10);
			if (!isNaN(num)) {
				if (num < 0) num = 0;
				if (num > 4294967295) num = 4294967295;
				target.value = String(num);
			}
			setPendingEdit(key, isNaN(num) ? undefined : num);
		} else {
			setPendingEdit(key, val);
		}
	}
</script>

<svelte:window onpaste={onPaste} />

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
				{@const st = fieldStates[field.key]}
				<div class="field-group">
					<label class="field-label" for="tag-{field.key}">{field.label}</label>
					<input
						id="tag-{field.key}"
						class="field-input"
						class:keep={st.keep}
						class:edited={st.edited}
						type="text"
						value={st.keep ? '' : st.value}
						placeholder={st.keep ? '‹ keep ›' : '—'}
						onchange={(e) => handleInput(field.key, e)}
					/>
				</div>
			{/each}
		{/if}
	</div>

	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="cover-area"
		class:drag-over={dragOver}
		ondragover={(e) => { e.preventDefault(); dragOver = true; }}
		ondragenter={(e) => { e.preventDefault(); dragOver = true; }}
		ondragleave={() => { dragOver = false; }}
		ondrop={onDrop}
	>
		{#if coverArtUrl && !coverArtError}
			<img
				src={coverArtUrl}
				alt="Cover art for {$selectedFiles[0]?.filename ?? 'selected file'}"
				class="cover-img"
				loading="lazy"
				onerror={() => (coverArtError = true)}
			/>
		{:else}
			<div class="cover-placeholder">
				<span class="cover-text">{uploading ? 'Uploading...' : 'No Cover'}</span>
			</div>
		{/if}
		{#if $selectedCount > 0}
			<div class="cover-actions">
				<button class="cover-btn" onclick={() => fileInput?.click()} disabled={uploading}>
					{uploading ? '...' : 'Add'}
				</button>
			</div>
			<input
				bind:this={fileInput}
				type="file"
				accept="image/jpeg,image/png"
				onchange={onFileSelected}
				style="display:none"
			/>
		{/if}
		{#if dragOver}
			<div class="drop-overlay">
				<span>Drop image</span>
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
		border-radius: var(--radius-md);
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

	.field-input:focus-visible {
		border-color: var(--accent);
		box-shadow: 0 0 0 1px var(--accent);
	}

	.field-input.edited {
		border-left: 2px solid var(--state-dirty);
		color: var(--state-dirty);
	}

	.field-input.keep {
		color: var(--text-muted);
		font-style: italic;
	}

	.field-input.keep::placeholder {
		color: var(--text-muted);
		font-style: italic;
	}

	.cover-area {
		padding: 10px 12px 12px;
		flex-shrink: 0;
		border-top: 1px solid var(--border-subtle);
		position: relative;
	}

	.cover-placeholder {
		aspect-ratio: 1;
		background: var(--bg-base);
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		box-shadow: var(--shadow-inset);
	}

	.cover-img {
		width: 100%;
		aspect-ratio: 1;
		object-fit: cover;
		border-radius: var(--radius-sm);
	}

	.cover-text {
		color: var(--text-muted);
		font-size: 11px;
	}

	.cover-area.drag-over {
		outline: 2px dashed var(--accent);
		outline-offset: -2px;
		border-radius: var(--radius-sm);
	}

	.cover-actions {
		display: flex;
		gap: 4px;
		margin-top: 6px;
	}

	.cover-btn {
		flex: 1;
		background: var(--bg-elevated);
		border: 1px solid var(--border-subtle);
		color: var(--text-secondary);
		font-size: 11px;
		font-family: var(--font-ui);
		padding: 3px 8px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		transition: background 0.1s, color 0.1s;
	}

	.cover-btn:hover:not(:disabled) {
		background: var(--accent-subtle);
		color: var(--accent);
	}

	.cover-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.drop-overlay {
		position: absolute;
		inset: 0;
		background: var(--accent-15);
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--accent);
		font-size: 12px;
		font-weight: 600;
		pointer-events: none;
	}
</style>
