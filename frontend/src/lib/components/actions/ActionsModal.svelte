<script lang="ts">
	import Modal from '$lib/components/common/Modal.svelte';
	import type { FileEntry } from '$lib/types/audio';
	import { previewActions, executeActions } from '$lib/api/actions';
	import type { Action, ActionPreview } from '$lib/api/actions';
	import { fetchTagsForFiles } from '$lib/stores/tags';

	interface Props {
		open: boolean;
		onClose: () => void;
		files: FileEntry[];
		onComplete: () => void;
	}

	let { open, onClose, files, onComplete }: Props = $props();

	let actions = $state<Action[]>([]);
	let previews = $state<ActionPreview[]>([]);
	let loading = $state(false);
	let executing = $state(false);
	let previewTaskId = 0;

	// New action form
	let actionType = $state('case_conversion');
	let actionField = $state('title');
	let caseMode = $state('title');
	let searchText = $state('');
	let replaceText = $state('');
	let useRegex = $state(false);
	let formatStr = $state('');
	let setValue = $state('');
	let autoStart = $state(1);
	let autoPadding = $state(2);

	function addAction() {
		let action: Action;
		switch (actionType) {
			case 'case_conversion':
				action = { type: 'case_conversion', field: actionField, mode: caseMode };
				break;
			case 'replace':
				action = { type: 'replace', field: actionField, search: searchText, replace: replaceText, regex: useRegex };
				break;
			case 'format_value':
				action = { type: 'format_value', field: actionField, format: formatStr };
				break;
			case 'set_field':
				action = { type: 'set_field', field: actionField, value: setValue };
				break;
			case 'remove_field':
				action = { type: 'remove_field', field: actionField };
				break;
			case 'auto_number':
				action = { type: 'auto_number', field: actionField, start: autoStart, padding: autoPadding };
				break;
			case 'trim_field':
				action = { type: 'trim_field', field: actionField };
				break;
			default:
				return;
		}
		actions = [...actions, action];
		loadPreview();
	}

	function removeAction(index: number) {
		actions = actions.filter((_, i) => i !== index);
		if (actions.length > 0) loadPreview();
		else previews = [];
	}

	async function loadPreview() {
		if (actions.length === 0 || files.length === 0) {
			previews = [];
			return;
		}
		const taskId = ++previewTaskId;
		loading = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await previewActions(fileEntries, actions);
			if (taskId === previewTaskId) {
				previews = results;
			}
		} catch (err) {
			console.error('Actions preview failed:', err);
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
		if (actions.length === 0) return;
		executing = true;
		try {
			const fileEntries = files.map((f) => ({ id: f.id, path: f.relative_path }));
			const results = await executeActions(fileEntries, actions);
			const failed = results.filter((r) => r.status === 'error');
			if (failed.length > 0) {
				console.warn(`Actions: ${results.length - failed.length} ok, ${failed.length} failed`);
			}
			const okIds = results.filter((r) => r.status === 'ok').map((r) => r.id);
			if (okIds.length > 0) {
				await fetchTagsForFiles(okIds, true);
			}
			onComplete();
			onClose();
		} catch (err) {
			console.error('Actions execution failed:', err);
		} finally {
			executing = false;
		}
	}

	function describeAction(a: Action): string {
		switch (a.type) {
			case 'case_conversion': return `${a.mode} case → ${a.field}`;
			case 'replace': return `Replace in ${a.field}: "${a.search}" → "${a.replace}"${a.regex ? ' (regex)' : ''}`;
			case 'format_value': return `Format ${a.field}: ${a.format}`;
			case 'set_field': return `Set ${a.field} = "${a.value}"`;
			case 'remove_field': return `Remove ${a.field}`;
			case 'auto_number': return `Auto-number ${a.field} (start: ${a.start}, pad: ${a.padding})`;
			case 'trim_field': return `Trim ${a.field}`;
			default: return a.type;
		}
	}

	const FIELDS = ['title', 'artist', 'album', 'album_artist', 'year', 'track_number', 'track_total', 'disc_number', 'disc_total', 'genre', 'comment', 'composer'];
	const ACTION_TYPES = [
		{ value: 'case_conversion', label: 'Case Conversion' },
		{ value: 'replace', label: 'Replace' },
		{ value: 'format_value', label: 'Format Value' },
		{ value: 'set_field', label: 'Set Field' },
		{ value: 'remove_field', label: 'Remove Field' },
		{ value: 'auto_number', label: 'Auto-Number' },
		{ value: 'trim_field', label: 'Trim Whitespace' },
	];
</script>

<Modal title="Actions" {open} {onClose} wide>
	<div class="actions-layout">
		<!-- Action builder -->
		<div class="builder">
			<div class="builder-row">
				<select class="input" bind:value={actionType}>
					{#each ACTION_TYPES as t}
						<option value={t.value}>{t.label}</option>
					{/each}
				</select>
				<select class="input" bind:value={actionField}>
					{#each FIELDS as f}
						<option value={f}>{f}</option>
					{/each}
				</select>
			</div>

			{#if actionType === 'case_conversion'}
				<select class="input" bind:value={caseMode}>
					<option value="title">Title Case</option>
					<option value="upper">UPPER CASE</option>
					<option value="lower">lower case</option>
					<option value="sentence">Sentence case</option>
				</select>
			{:else if actionType === 'replace'}
				<div class="builder-row">
					<input class="input" bind:value={searchText} placeholder="Search for..." />
					<input class="input" bind:value={replaceText} placeholder="Replace with..." />
					<label class="checkbox-label">
						<input type="checkbox" bind:checked={useRegex} /> Regex
					</label>
				</div>
			{:else if actionType === 'format_value'}
				<input class="input mono" bind:value={formatStr} placeholder="$upper(%artist%) - %title%" />
			{:else if actionType === 'set_field'}
				<input class="input" bind:value={setValue} placeholder="Value..." />
			{:else if actionType === 'auto_number'}
				<div class="builder-row">
					<label class="inline-label">Start: <input class="input input-sm" type="number" bind:value={autoStart} min="0" /></label>
					<label class="inline-label">Padding: <input class="input input-sm" type="number" bind:value={autoPadding} min="1" max="6" /></label>
				</div>
			{/if}

			<button class="btn btn-secondary btn-sm" onclick={addAction}>+ Add Action</button>
		</div>

		<!-- Action chain -->
		{#if actions.length > 0}
			<div class="chain">
				<div class="chain-header">Action Chain ({actions.length})</div>
				{#each actions as action, i}
					<div class="chain-item">
						<span class="chain-num">{i + 1}.</span>
						<span class="chain-desc">{describeAction(action)}</span>
						<button class="chain-remove" onclick={() => removeAction(i)} title="Remove">&times;</button>
					</div>
				{/each}
			</div>
		{/if}

		<!-- Preview -->
		{#if loading}
			<div class="preview-loading"><span class="spinner spinner--sm"></span> Loading preview...</div>
		{:else if previews.length > 0}
			<div class="preview-list">
				<div class="preview-header">Preview: {previews.length} file{previews.length !== 1 ? 's' : ''} will change</div>
				{#each previews.slice(0, 20) as p (p.id)}
					<div class="preview-item">
						<span class="preview-filename mono">{p.filename}</span>
						{#each p.changes as c}
							<div class="preview-change">
								<span class="change-field">{c.field}:</span>
								<span class="change-old">{c.old_value || '(empty)'}</span>
								<span class="change-arrow">&rarr;</span>
								<span class="change-new">{c.new_value || '(empty)'}</span>
							</div>
						{/each}
					</div>
				{/each}
				{#if previews.length > 20}
					<div class="preview-more">...and {previews.length - 20} more</div>
				{/if}
			</div>
		{:else if actions.length > 0}
			<div class="preview-empty">No changes detected</div>
		{/if}
	</div>

	<div class="modal-actions">
		<button class="btn btn-secondary" onclick={onClose}>Cancel</button>
		<button
			class="btn btn-primary"
			disabled={actions.length === 0 || executing}
			onclick={handleExecute}
		>
			{executing ? 'Applying...' : `Apply to ${files.length} file${files.length !== 1 ? 's' : ''}`}
		</button>
	</div>
</Modal>

<style>
	.actions-layout {
		display: flex;
		flex-direction: column;
		gap: 12px;
		margin-bottom: 12px;
	}

	.builder {
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 8px;
		background: var(--bg-base);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}

	.builder-row {
		display: flex;
		gap: 6px;
		align-items: center;
	}

	.input {
		background: var(--bg-surface);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-size: 12px;
		padding: 4px 8px;
		border-radius: var(--radius-sm);
		outline: none;
		flex: 1;
	}

	.input:focus {
		border-color: var(--accent);
	}

	.input-sm {
		width: 60px;
		flex: none;
	}

	.checkbox-label {
		font-size: 11px;
		color: var(--text-secondary);
		display: flex;
		align-items: center;
		gap: 4px;
		white-space: nowrap;
	}

	.inline-label {
		font-size: 11px;
		color: var(--text-secondary);
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.btn-sm {
		font-size: 11px;
		padding: 3px 10px;
		align-self: flex-start;
	}

	.chain {
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}

	.chain-header {
		padding: 4px 8px;
		background: var(--grid-header-bg);
		font-size: 10px;
		color: var(--text-muted);
		font-weight: 600;
		text-transform: uppercase;
	}

	.chain-item {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 8px;
		border-bottom: 1px solid var(--grid-border);
		font-size: 11px;
	}

	.chain-num {
		color: var(--text-muted);
		font-weight: 600;
		width: 18px;
	}

	.chain-desc {
		flex: 1;
		color: var(--text-secondary);
	}

	.chain-remove {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 14px;
		padding: 0 4px;
	}

	.chain-remove:hover {
		color: var(--error);
	}

	.preview-list {
		max-height: 200px;
		overflow-y: auto;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
	}

	.preview-header {
		padding: 4px 8px;
		background: var(--grid-header-bg);
		font-size: 10px;
		color: var(--text-muted);
		font-weight: 600;
	}

	.preview-item {
		padding: 4px 8px;
		border-bottom: 1px solid var(--grid-border);
	}

	.preview-filename {
		font-size: 11px;
		color: var(--text-secondary);
	}

	.preview-change {
		display: flex;
		gap: 4px;
		font-size: 10px;
		padding-left: 12px;
		align-items: baseline;
	}

	.change-field {
		color: var(--text-muted);
		font-weight: 600;
		min-width: 80px;
	}

	.change-old {
		color: var(--text-muted);
		text-decoration: line-through;
	}

	.change-arrow {
		color: var(--text-muted);
	}

	.change-new {
		color: var(--accent);
	}

	.preview-loading, .preview-empty {
		color: var(--text-muted);
		font-size: 12px;
		padding: 12px;
		text-align: center;
	}

	.preview-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
	}

	.preview-more {
		padding: 4px 8px;
		font-size: 10px;
		color: var(--text-muted);
		font-style: italic;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
