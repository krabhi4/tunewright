<script lang="ts">
	import Icon from '$lib/icons/Icon.svelte';
	import { ICONS } from '$lib/icons';
	import { themeFamily, themeMode, setThemeFamily, setThemeMode } from '$lib/stores/theme';
	import { THEME_FAMILIES, familySupportsLight, type ThemeFamily } from '$lib/theme/resolve';

	const FAMILY_META: Record<ThemeFamily, { label: string; swatch: string; hint: string }> = {
		console: { label: 'Console', swatch: '#4d8dff', hint: 'Utility · default' },
		editorial: { label: 'Editorial', swatch: '#d2734a', hint: 'Warm · serif' },
		terminal: { label: 'Terminal', swatch: '#f0a830', hint: 'Mono · dark' },
		daw: { label: 'DAW', swatch: '#34e0e0', hint: 'Pro audio · dark' }
	};

	let open = $state(false);
	let menuEl = $state<HTMLDivElement>();

	function toggle() {
		open = !open;
	}

	function pickFamily(f: ThemeFamily) {
		setThemeFamily(f);
	}

	function onWindowClick(e: MouseEvent) {
		if (open && menuEl && !menuEl.contains(e.target as Node)) open = false;
	}

	function onKey(e: KeyboardEvent) {
		if (e.key === 'Escape') open = false;
	}
</script>

<svelte:window onclick={onWindowClick} onkeydown={onKey} />

<div class="theme-menu" bind:this={menuEl}>
	<button
		class="theme-trigger"
		onclick={toggle}
		title="Theme"
		aria-haspopup="menu"
		aria-expanded={open}
		aria-label="Choose theme"
	>
		<Icon path={ICONS.palette} size={16} />
	</button>

	{#if open}
		<div class="menu" role="menu">
			<div class="menu-label">Theme</div>
			{#each THEME_FAMILIES as f (f)}
				<button
					class="menu-item"
					class:active={$themeFamily === f}
					role="menuitemradio"
					aria-checked={$themeFamily === f}
					onclick={() => pickFamily(f)}
				>
					<span class="swatch" style="background: {FAMILY_META[f].swatch}"></span>
					<span class="text">
						<span class="name">{FAMILY_META[f].label}</span>
						<span class="hint">{FAMILY_META[f].hint}</span>
					</span>
					{#if $themeFamily === f}
						<span class="check"><Icon path={ICONS.check} size={14} /></span>
					{/if}
				</button>
			{/each}

			<div class="menu-sep"></div>
			<div class="menu-label">Appearance</div>
			{#if familySupportsLight($themeFamily)}
				<div class="mode-row">
					<button class="mode-btn" class:active={$themeMode === 'dark'} onclick={() => setThemeMode('dark')}>
						<Icon path={ICONS.moon} size={14} /> Dark
					</button>
					<button class="mode-btn" class:active={$themeMode === 'light'} onclick={() => setThemeMode('light')}>
						<Icon path={ICONS.sun} size={14} /> Light
					</button>
				</div>
			{:else}
				<div class="mode-note">Dark only</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.theme-menu {
		position: relative;
		display: flex;
		align-items: center;
		margin-right: 6px;
	}

	.theme-trigger {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 4px 6px;
		background: transparent;
		border: none;
		color: var(--text-secondary);
		cursor: pointer;
		border-radius: var(--radius-sm);
		transition: background 0.1s, color 0.1s;
	}

	.theme-trigger:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.menu {
		position: absolute;
		top: calc(100% + 6px);
		right: 0;
		min-width: 200px;
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-dropdown);
		padding: 5px;
		z-index: var(--z-dropdown);
	}

	.menu-label {
		font-size: 9px;
		font-weight: 600;
		letter-spacing: 0.07em;
		text-transform: uppercase;
		color: var(--text-muted);
		padding: 5px 7px 3px;
	}

	.menu-item {
		display: flex;
		align-items: center;
		gap: 9px;
		width: 100%;
		padding: 6px 7px;
		background: transparent;
		border: none;
		border-radius: var(--radius-sm);
		cursor: pointer;
		color: var(--text-secondary);
		font-family: var(--font-ui);
		text-align: left;
	}

	.menu-item:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.menu-item.active {
		color: var(--text-primary);
	}

	.swatch {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		flex-shrink: 0;
		box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.15);
	}

	.text {
		display: flex;
		flex-direction: column;
		line-height: 1.25;
		flex: 1;
		min-width: 0;
	}

	.name {
		font-size: 12.5px;
	}

	.hint {
		font-size: 10px;
		color: var(--text-muted);
	}

	.check {
		display: flex;
		color: var(--accent);
		flex-shrink: 0;
	}

	.menu-sep {
		height: 1px;
		background: var(--border-subtle);
		margin: 5px 2px;
	}

	.mode-row {
		display: flex;
		gap: 4px;
		padding: 2px 2px 3px;
	}

	.mode-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 5px;
		flex: 1;
		padding: 5px 8px;
		background: var(--bg-base);
		border: 1px solid var(--border-subtle);
		border-radius: var(--radius-sm);
		cursor: pointer;
		color: var(--text-secondary);
		font-family: var(--font-ui);
		font-size: 11.5px;
	}

	.mode-btn:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.mode-btn.active {
		background: var(--accent-subtle);
		border-color: var(--accent-muted);
		color: var(--accent);
	}

	.mode-note {
		padding: 4px 9px 7px;
		font-size: 11px;
		color: var(--text-muted);
		font-style: italic;
	}
</style>
