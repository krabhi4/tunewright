<script lang="ts">
	import UserMenu from './UserMenu.svelte';

	interface Props {
		onOpenFolder: () => void;
		onSave?: () => void;
		onRename?: () => void;
		onLookup?: () => void;
		onManageUsers?: () => void;
		hasPendingEdits?: boolean;
		hasSelection?: boolean;
	}

	let { onOpenFolder, onSave, onRename, onLookup, onManageUsers, hasPendingEdits = false, hasSelection = false }: Props = $props();
</script>

<div class="toolbar">
	<div class="toolbar-group">
		<button class="toolbar-btn" onclick={onOpenFolder}>
			<span class="toolbar-icon">&#128193;</span>
			<span>Open</span>
		</button>

		<button class="toolbar-btn" onclick={onSave} disabled={!hasPendingEdits}>
			<span class="toolbar-icon">&#128190;</span>
			<span>Save</span>
		</button>
	</div>

	<div class="toolbar-group">
		<button class="toolbar-btn" disabled={!hasSelection} onclick={onRename}>
			<span class="toolbar-icon">&#9998;</span>
			<span>Rename</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onLookup}>
			<span class="toolbar-icon">&#128269;</span>
			<span>Lookup</span>
		</button>
	</div>

	<div class="toolbar-spacer"></div>

	<div class="toolbar-title">TagStudio</div>

	<UserMenu {onManageUsers} />
</div>

<style>
	.toolbar {
		height: var(--toolbar-height);
		background: var(--bg-surface);
		border-bottom: 1px solid var(--border);
		display: flex;
		align-items: center;
		padding: 0 10px;
		gap: 2px;
		flex-shrink: 0;
		user-select: none;
	}

	.toolbar-group {
		display: flex;
		gap: 1px;
	}

	.toolbar-btn {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 4px 10px;
		background: transparent;
		border: none;
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 12px;
		cursor: pointer;
		border-radius: var(--radius-sm);
		transition: background 0.1s, color 0.1s;
	}

	.toolbar-btn:hover:not(:disabled) {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.toolbar-btn:active:not(:disabled) {
		background: var(--accent-muted);
	}

	.toolbar-btn:disabled {
		opacity: 0.35;
		cursor: default;
	}

	.toolbar-icon {
		font-size: 14px;
		width: 18px;
		text-align: center;
	}

	.toolbar-spacer {
		flex: 1;
	}

	.toolbar-title {
		font-size: 11px;
		font-weight: 500;
		color: var(--text-muted);
		letter-spacing: 0.5px;
		text-transform: uppercase;
		margin-right: 8px;
	}
</style>
