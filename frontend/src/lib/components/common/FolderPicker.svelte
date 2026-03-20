<script lang="ts">
	import Modal from './Modal.svelte';

	interface Props {
		open: boolean;
		onClose: () => void;
		onSelect: (path: string) => void;
	}

	let { open, onClose, onSelect }: Props = $props();

	let manualPath = $state('/');

	function handleOpen() {
		const path = manualPath.trim() || '/';
		onSelect(path);
		onClose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') handleOpen();
	}
</script>

<Modal title="Open Folder" {open} {onClose}>
	<div class="help-text">
		Enter a path relative to the data root. Use the path bar and folder rows in the grid to navigate.
	</div>
	<div class="path-row">
		<input
			class="path-input"
			type="text"
			bind:value={manualPath}
			onkeydown={handleKeydown}
			placeholder="/ or /Music/Rock"
			aria-label="Folder path"
		/>
		<button class="path-btn" onclick={handleOpen}>Open</button>
	</div>
</Modal>

<style>
	.help-text {
		color: var(--text-muted);
		font-size: 11.5px;
		margin-bottom: 10px;
		line-height: 1.5;
	}

	.path-row {
		display: flex;
		gap: 6px;
	}

	.path-input {
		flex: 1;
		background: var(--bg-base);
		border: 1px solid var(--border);
		color: var(--text-primary);
		font-family: var(--font-mono);
		font-size: 13px;
		padding: 8px 10px;
		border-radius: var(--radius-sm);
		outline: none;
	}

	.path-input:focus {
		border-color: var(--accent);
	}

	.path-btn {
		background: var(--accent);
		border: none;
		color: var(--text-on-accent);
		padding: 8px 18px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-family: var(--font-ui);
		font-size: 13px;
		font-weight: 500;
	}

	.path-btn:hover {
		background: var(--accent-hover);
	}
</style>
