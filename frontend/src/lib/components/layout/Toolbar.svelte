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
		<button class="toolbar-btn" onclick={onOpenFolder} title="Open folder">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4.5V12a1 1 0 001 1h10a1 1 0 001-1V6a1 1 0 00-1-1H8L6.5 3.5H3a1 1 0 00-1 1z"/></svg>
			<span>Open</span>
		</button>

		<button class="toolbar-btn" onclick={onSave} disabled={!hasPendingEdits} title="Save all tag edits (Ctrl+S)">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 2h8l2 2v9a1 1 0 01-1 1H3a1 1 0 01-1-1V3a1 1 0 011-1z"/><path d="M5 2v3h5V2"/><path d="M5 10h6"/><path d="M5 12.5h4"/></svg>
			<span>Save</span>
		</button>
	</div>

	<div class="toolbar-group">
		<button class="toolbar-btn" disabled={!hasSelection} onclick={onRename} title="Rename selected files">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 3.5l3 3M3 13l-.5-3L10 2.5l3 3L5.5 13z"/></svg>
			<span>Rename</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onLookup} title="Look up tags on MusicBrainz">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="6.5" cy="6.5" r="4"/><path d="M14 14l-4-4"/></svg>
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
		width: 15px;
		height: 15px;
		flex-shrink: 0;
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

	@media (max-width: 768px) {
		.toolbar-btn {
			padding: 10px 12px;
			min-height: 44px;
		}

		.toolbar-btn span:not(.toolbar-icon) {
			display: none;
		}

		.toolbar-title {
			display: none;
		}
	}
</style>
