<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry } from '$lib/types/audio';
	import { previewRenames, executeRenames } from '$lib/api/rename';
	import type { RenamePreview } from '$lib/api/rename';

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
		loading = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			previews = await previewRenames(fileEntries, format);
		} catch (err) {
			console.error('Preview failed:', err);
			previews = [];
		} finally {
			loading = false;
		}
	}

	async function handleExecute() {
		executing = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await executeRenames(fileEntries, format);
			const failed = results.filter((r) => r.status === 'error');
			if (failed.length > 0) {
				console.warn(`Rename: ${results.length - failed.length} ok, ${failed.length} failed`);
			}
			onComplete();
			onClose();
		} catch (err) {
			console.error('Rename failed:', err);
		} finally {
			executing = false;
		}
	}

	let hasConflicts = $derived(previews.some((p) => p.conflict));

	function handleFormatChange() {
		loadPreview();
	}
</script>

<Modal title="Rename Files" {open} {onClose}>
	<div class="rename-form">
		<!-- svelte-ignore a11y_label_has_associated_control -->
		<label class="label">Format String</label>
		<input
			class="format-input"
			type="text"
			bind:value={format}
			onchange={handleFormatChange}
			placeholder="%track% - %artist% - %title%"
		/>
		<div class="tokens">
			{#each ['%track%', '%artist%', '%title%', '%album%', '%year%', '%genre%'] as token}
				<button class="token-btn" onclick={() => { format += token; loadPreview(); }}>
					{token}
				</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="preview-loading">Loading preview...</div>
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
			<span class="conflict-warning">Conflicts detected</span>
		{/if}
		<button class="btn-cancel" onclick={onClose}>Cancel</button>
		<button
			class="btn-execute"
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
		background: rgba(239, 68, 68, 0.1);
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

	.mono {
		font-family: var(--font-mono);
		font-size: 11px;
	}

	.preview-loading {
		color: var(--text-muted);
		font-size: 12px;
		padding: 12px;
		text-align: center;
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

	.btn-cancel {
		background: transparent;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		padding: 5px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
	}

	.btn-cancel:hover {
		background: var(--bg-hover);
	}

	.btn-execute {
		background: var(--accent);
		border: none;
		color: white;
		padding: 5px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 12px;
	}

	.btn-execute:hover:not(:disabled) {
		background: var(--accent-hover);
	}

	.btn-execute:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>
