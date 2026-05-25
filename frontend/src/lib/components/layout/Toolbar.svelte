<script lang="ts">
	import UserMenu from './UserMenu.svelte';
	import { theme, toggleTheme } from '$lib/stores/theme';

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
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 4.5V12a1 1 0 001 1h10a1 1 0 001-1V6a1 1 0 00-1-1H8L6.5 3.5H3a1 1 0 00-1 1z"/></svg>
			<span>Open</span>
		</button>

		<button class="toolbar-btn" onclick={onSave} disabled={!hasPendingEdits} title="Save all tag edits (Ctrl+S)">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 2h8l2 2v9a1 1 0 01-1 1H3a1 1 0 01-1-1V3a1 1 0 011-1z"/><path d="M5 2v3h5V2"/><path d="M5 10h6"/><path d="M5 12.5h4"/></svg>
			<span>Save</span>
		</button>
	</div>

	<div class="toolbar-group">
		<button class="toolbar-btn" disabled={!hasSelection} onclick={onRename} title="Rename selected files (Tag → Filename)">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 3.5l3 3M3 13l-.5-3L10 2.5l3 3L5.5 13z"/></svg>
			<span>Rename</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onFilenameToTag} title="Extract tags from filenames (Filename → Tag)">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2 3h12M2 7h8M2 11h5"/><path d="M12 8l2 3-2 3"/></svg>
			<span>Fn→Tag</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onActions} title="Apply batch actions to selected files">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M4 4h8M4 8h8M4 12h5"/><path d="M12 10l2 2-2 2"/></svg>
			<span>Actions</span>
		</button>

		<button class="toolbar-btn" disabled={!hasSelection} onclick={onLookup} title="Look up tags on MusicBrainz">
			<svg class="toolbar-icon" aria-hidden="true" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="6.5" cy="6.5" r="4"/><path d="M14 14l-4-4"/></svg>
			<span>Lookup</span>
		</button>
	</div>

	<div class="toolbar-spacer"></div>

	<div class="toolbar-title">
		<svg class="toolbar-logo" viewBox="0 0 32 32" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
			<path d="M10 12 L16 6 L22 12 L22 25 A 2 2 0 0 1 20 27 L12 27 A 2 2 0 0 1 10 25 Z"/>
			<circle cx="16" cy="12" r="1.5" fill="currentColor"/>
			<rect x="12.25" y="16.5" width="1.5" height="6" rx="0.75" fill="currentColor"/>
			<rect x="15.25" y="14" width="1.5" height="10" rx="0.75" fill="currentColor"/>
			<rect x="18.25" y="17.5" width="1.5" height="5" rx="0.75" fill="currentColor"/>
		</svg>
		<span>TagStudio</span>
	</div>

	<button class="toolbar-btn theme-toggle-btn" onclick={toggleTheme} title="Switch Dark/Light Theme" aria-label="Toggle theme">
		{#if $theme === 'dark'}
			<!-- Sun Icon -->
			<svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<circle cx="12" cy="12" r="5"></circle>
				<line x1="12" y1="1" x2="12" y2="3"></line>
				<line x1="12" y1="21" x2="12" y2="23"></line>
				<line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
				<line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
				<line x1="1" y1="12" x2="3" y2="12"></line>
				<line x1="21" y1="12" x2="23" y2="12"></line>
				<line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
				<line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
			</svg>
		{:else}
			<!-- Moon Icon -->
			<svg class="toolbar-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
				<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
			</svg>
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
