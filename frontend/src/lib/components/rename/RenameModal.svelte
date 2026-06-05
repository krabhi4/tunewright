<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry } from '$lib/types/audio';
	import { previewRenames, executeRenames } from '$lib/api/rename';
	import type { RenamePreview } from '$lib/api/rename';
	import { toast } from '$lib/stores/toast';

	interface Props {
		open: boolean;
		onClose: () => void;
		files: FileEntry[];
		onComplete: () => void;
	}

	let { open, onClose, files, onComplete }: Props = $props();

	let format = $state('%track% - %artist% - %title%');
	let previews = $state<RenamePreview[]>([]);
	let loading = $state(false);
	let executing = $state(false);
	let previewTaskId = 0;

	$effect(() => {
		if (open && files.length > 0) {
			loadPreview();
		}
	});

	async function loadPreview() {
		if (!format.trim()) {
			previews = [];
			return;
		}
		const taskId = ++previewTaskId;
		loading = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await previewRenames(fileEntries, format);
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

	async function handleExecute() {
		executing = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await executeRenames(fileEntries, format);
			const failed = results.filter((r) => r.status === 'error');
			const ok = results.filter((r) => r.status === 'ok').length;
			if (failed.length > 0) {
				console.warn(`Rename: ${ok} ok, ${failed.length} failed`);
				toast.warning(`Renamed ${ok} file(s); ${failed.length} failed.`);
			} else if (ok > 0) {
				toast.success(`Renamed ${ok} file(s).`);
			}
			onComplete();
			onClose();
		} catch (err) {
			console.error('Rename failed:', err);
			toast.error('Rename failed. See console for details.');
		} finally {
			executing = false;
		}
	}

	let hasConflicts = $derived(previews.some((p) => p.conflict));

	let previewTimer: ReturnType<typeof setTimeout>;

	function handleFormatInput() {
		clearTimeout(previewTimer);
		previewTimer = setTimeout(() => loadPreview(), 300);
	}
</script>

<Modal title="Rename Files" {open} {onClose} wide={true}>
	<div class="rename-form">
		<label class="label" for="rename-format">Format String</label>
		<input
			id="rename-format"
			class="format-input"
			type="text"
			bind:value={format}
			oninput={handleFormatInput}
			placeholder="%track% - %artist% - %title%"
		/>
		<div class="tokens" role="group" aria-label="Format tokens">
			{#each ['%track%', '%artist%', '%title%', '%album%', '%year%', '%genre%'] as token}
				<button class="token-btn" onclick={() => { format += token; loadPreview(); }}>
					{token}
				</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="preview-loading"><span class="spinner spinner--sm"></span> Loading preview...</div>
	{:else if previews.length > 0}
		<div class="preview-list">
			<div class="preview-header">
				<span class="preview-col">Current</span>
				<span class="preview-arrow"></span>
				<span class="preview-col">New</span>
			</div>
			{#each previews as p (p.id)}
				<div class="preview-row" class:conflict={p.conflict}>
					<span class="preview-old mono">{p.old_name}</span>
					<span class="preview-arrow">&rarr;</span>
					<span class="preview-new mono" class:changed={p.old_name !== p.new_name}>
						{p.new_name}
					</span>
				</div>
			{/each}
		</div>
	{/if}

	<div class="rename-actions">
		{#if hasConflicts}
			<span class="conflict-warning" role="alert">Conflicts detected</span>
		{/if}
		<button class="btn btn-secondary" onclick={onClose}>Cancel</button>
		<button
			class="btn btn-primary"
			disabled={previews.length === 0 || hasConflicts || executing}
			onclick={handleExecute}
		>
			{executing ? 'Renaming...' : 'Rename'}
		</button>
	</div>
</Modal>

<style>
	.rename-form {
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
		overflow-y: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		margin-bottom: 12px;
	}

	.preview-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 4px 8px;
		background: var(--grid-header-bg);
		font-size: 10px;
		color: var(--text-muted);
		font-weight: 600;
		text-transform: uppercase;
	}

	.preview-col {
		flex: 1;
	}

	.preview-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 3px 8px;
		border-bottom: 1px solid var(--grid-border);
		font-size: 11px;
	}

	.preview-row.conflict {
		background: var(--error-10);
	}

	.preview-old {
		flex: 1;
		color: var(--text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-arrow {
		color: var(--text-muted);
		font-size: 11px;
		flex-shrink: 0;
		width: 20px;
		text-align: center;
	}

	.preview-new {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-new.changed {
		color: var(--accent);
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

	.rename-actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: 8px;
	}

	.conflict-warning {
		color: var(--error);
		font-size: 11px;
		margin-right: auto;
	}

</style>
