<script lang="ts">
	import UserMenu from './UserMenu.svelte';
	import { themeMode, toggleTheme } from '$lib/stores/theme';
	import Icon from '$lib/icons/Icon.svelte';
	import { ICONS } from '$lib/icons';
	import Logo from '$lib/icons/Logo.svelte';

	interface Props {
		onOpenFolder: () => void;
		onSave?: () => void;
		onRename?: () => void;
		onFilenameToTag?: () => void;
		onActions?: () => void;
		onLookup?: () => void;
		onManageUsers?: () => void;
		hasPendingEdits?: boolean;
		hasSelection?: boolean;
	}

	let { onOpenFolder, onSave, onRename, onFilenameToTag, onActions, onLookup, onManageUsers, hasPendingEdits = false, hasSelection = false }: Props = $props();
</script>

<div class="toolbar">
	<div class="toolbar-group">
		<button class="toolbar-btn" onclick={onOpenFolder} title="Open folder">
			<Icon path={ICONS.open} size={15} />
			<span>Open</span>
		</button>

		<button class="toolbar-btn" onclick={onSave} disabled={!hasPendingEdits} title="Save all tag edits (Ctrl+S)">
			<Icon path={ICONS.save} size={15} />
			<span>Save</span>
		</button>
	</div>

	<div class="toolbar-group">
		<button class="toolbar-btn" disabled={!hasSelection} onclick={onRename} title="Rename selected files (Tag → Filename)">
			<Icon path={ICONS.rename} size={15} />
			<span>Rename</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onFilenameToTag} title="Extract tags from filenames (Filename → Tag)">
			<Icon path={ICONS.fnToTag} size={15} />
			<span>Fn→Tag</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onActions} title="Apply batch actions to selected files">
			<Icon path={ICONS.actions} size={15} />
			<span>Actions</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onLookup} title="Look up tags on MusicBrainz">
			<Icon path={ICONS.lookup} size={15} />
			<span>Lookup</span>
		</button>
	</div>

	<div class="toolbar-spacer"></div>

	<div class="toolbar-title">
		<Logo size={16} />
		<span>TagStudio</span>
	</div>

	<button class="toolbar-btn theme-toggle-btn" onclick={toggleTheme} title="Switch Dark/Light Theme" aria-label="Toggle theme">
		{#if $themeMode === 'dark'}
			<Icon path={ICONS.sun} size={16} label="Switch to light theme" />
		{:else}
			<Icon path={ICONS.moon} size={16} label="Switch to dark theme" />
		{/if}
	</button>

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
		display: flex;
		align-items: center;
		gap: 6px;
		font-size: 11px;
		font-weight: 600;
		color: var(--text-muted);
		letter-spacing: 0.5px;
		text-transform: uppercase;
		margin-right: 8px;
	}

	.toolbar-logo {
		width: 16px;
		height: 16px;
		color: var(--accent);
		flex-shrink: 0;
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

	.theme-toggle-btn {
		margin-right: 6px;
		padding: 4px 6px;
		border-radius: var(--radius-sm);
		display: flex;
		align-items: center;
		justify-content: center;
	}
</style>
