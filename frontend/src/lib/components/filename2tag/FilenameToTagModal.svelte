<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry, TagData } from '$lib/types/audio';
	import { previewFilenameToTag } from '$lib/api/filename-to-tag';
	import type { FilenameTagPreview } from '$lib/api/filename-to-tag';
	import { writeTags } from '$lib/api/tags';
	import { fetchTagsForFiles } from '$lib/stores/tags';
	import { toast } from '$lib/stores/toast';

	interface Props {
		open: boolean;
		onClose: () => void;
		files: FileEntry[];
		onComplete: () => void;
	}

	let { open, onClose, files, onComplete }: Props = $props();

	let pattern = $state('%artist% - %title%');
	let previews = $state<FilenameTagPreview[]>([]);
	let loading = $state(false);
	let applying = $state(false);
	let previewTaskId = 0;

	$effect(() => {
		if (open && files.length > 0) {
			loadPreview();
		}
	});

	async function loadPreview() {
		if (!pattern.trim()) {
			previews = [];
			return;
		}
		const taskId = ++previewTaskId;
		loading = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await previewFilenameToTag(fileEntries, pattern);
			if (taskId === previewTaskId) {
				previews = results;
			}
		} catch (err) {
			console.error('Preview failed:', err);
			if (taskId === previewTaskId) {
				previews = [];
			}
		} finally {
			if (taskId === previewTaskId) {
				loading = false;
			}
		}
	}

	async function handleApply() {
		const matched = previews.filter((p) => p.matched && p.tags);
		if (matched.length === 0) return;

		applying = true;
		try {
			const changes = matched.map((p) => {
				const file = files.find((f) => f.id === p.id);
				return {
					id: p.id,
					path: file?.relative_path ?? '',
					tags: p.tags as Partial<TagData>
				};
			}).filter((c) => c.path !== '');

			const results = await writeTags(changes);
			const failed = results.filter((r) => r.status === 'error');
			const okIds = results.filter((r) => r.status === 'ok').map((r) => r.id);
			if (failed.length > 0) {
				console.warn(`Filename-to-tag: ${okIds.length} ok, ${failed.length} failed`);
				toast.warning(`Applied tags to ${okIds.length} file(s); ${failed.length} failed.`);
			} else if (okIds.length > 0) {
				toast.success(`Applied tags to ${okIds.length} file(s).`);
			}

			// Refresh tags for updated files
			if (okIds.length > 0) {
				await fetchTagsForFiles(okIds, true);
			}

			onComplete();
			onClose();
		} catch (err) {
			console.error('Apply failed:', err);
			toast.error('Apply failed. See console for details.');
		} finally {
			applying = false;
		}
	}

	let matchCount = $derived(previews.filter((p) => p.matched).length);

	let previewTimer: ReturnType<typeof setTimeout>;

	function handlePatternInput() {
		clearTimeout(previewTimer);
		previewTimer = setTimeout(() => loadPreview(), 300);
	}

	function formatFieldValue(tags: Partial<TagData> | undefined, field: string): string {
		if (!tags) return '';
		return String((tags as Record<string, unknown>)[field] ?? '');
	}
</script>

<Modal title="Filename → Tag" {open} {onClose} wide={true}>
	<div class="form">
		<label class="label" for="f2t-pattern">Pattern</label>
		<input
			id="f2t-pattern"
			class="format-input"
			type="text"
			bind:value={pattern}
			oninput={handlePatternInput}
			placeholder="%artist% - %title%"
		/>
		<div class="tokens" role="group" aria-label="Pattern tokens">
			{#each ['%artist%', '%title%', '%album%', '%track%', '%year%', '%genre%'] as token}
				<button class="token-btn" onclick={() => { pattern += token; loadPreview(); }}>
					{token}
				</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="preview-loading"><span class="spinner spinner--sm"></span> Loading preview...</div>
	{:else if previews.length > 0}
		<div class="preview-list">
			<table class="preview-table">
				<thead>
					<tr>
						<th>Filename</th>
						<th>Artist</th>
						<th>Title</th>
						<th>Track</th>
						<th>Album</th>
					</tr>
				</thead>
				<tbody>
					{#each previews as p (p.id)}
						<tr class:unmatched={!p.matched}>
							<td class="mono filename-cell">{p.filename}</td>
							{#if p.matched && p.tags}
								<td class="extracted">{formatFieldValue(p.tags, 'artist')}</td>
								<td class="extracted">{formatFieldValue(p.tags, 'title')}</td>
								<td class="extracted">{formatFieldValue(p.tags, 'track_number')}</td>
								<td class="extracted">{formatFieldValue(p.tags, 'album')}</td>
							{:else}
								<td colspan="4" class="no-match">No match</td>
							{/if}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}

	<div class="actions">
		{#if previews.length > 0}
			<span class="match-info">
				{matchCount}/{previews.length} matched
			</span>
		{/if}
		<button class="btn btn-secondary" onclick={onClose}>Cancel</button>
		<button
			class="btn btn-primary"
			disabled={matchCount === 0 || applying}
			onclick={handleApply}
		>
			{applying ? 'Applying...' : `Apply to ${matchCount} file${matchCount !== 1 ? 's' : ''}`}
		</button>
	</div>
</Modal>

<style>
	.form {
		margin-bottom: 12px;
	}

	.label {
		display: block;
		font-size: 11px;
		color: var(--text-muted);
		margin-bottom: 4px;
	}

	.format-input {
		width: 100%;
		background: var(--bg-base);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 12px;
		padding: 6px 8px;
		border-radius: var(--radius-sm);
		outline: none;
	}

	.format-input:focus {
		border-color: var(--accent);
	}

	.tokens {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		margin-top: 6px;
	}

	.token-btn {
		font-size: 10px;
		font-family: var(--font-mono);
		color: var(--accent);
		background: var(--accent-subtle);
		border: none;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		cursor: pointer;
	}

	.token-btn:hover {
		background: var(--accent-muted);
	}

	.preview-list {
		max-height: 250px;
		overflow: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		margin-bottom: 12px;
	}

	.preview-table {
		width: 100%;
		border-collapse: collapse;
		font-size: 11px;
	}

	.preview-table th {
		position: sticky;
		top: 0;
		background: var(--grid-header-bg);
		color: var(--text-muted);
		font-size: 10px;
		font-weight: 600;
		text-transform: uppercase;
		text-align: left;
		padding: 4px 8px;
	}

	.preview-table td {
		padding: 3px 8px;
		border-bottom: 1px solid var(--grid-border);
	}

	.filename-cell {
		color: var(--text-secondary);
		max-width: 180px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.extracted {
		color: var(--accent);
	}

	.no-match {
		color: var(--text-muted);
		font-style: italic;
	}

	tr.unmatched {
		opacity: 0.5;
	}

	.preview-loading {
		color: var(--text-muted);
		font-size: 12px;
		padding: 12px;
		text-align: center;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 8px;
	}

	.match-info {
		color: var(--text-muted);
		font-size: 11px;
		margin-right: auto;
	}
</style>
