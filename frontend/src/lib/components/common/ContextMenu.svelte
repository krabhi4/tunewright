<script lang="ts">
	interface MenuItem {
		label: string;
		action: () => void;
		separator?: false;
	}

	interface SeparatorItem {
		separator: true;
	}

	interface Props {
		x: number;
		y: number;
		items: (MenuItem | SeparatorItem)[];
		onClose: () => void;
	}

	let { x, y, items, onClose }: Props = $props();

	let menuEl = $state<HTMLDivElement>();

	$effect(() => {
		if (menuEl) {
			const firstItem = menuEl.querySelector<HTMLElement>('[role="menuitem"]');
			firstItem?.focus();
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
			return;
		}

		if (!menuEl) return;
		const menuItems = Array.from(menuEl.querySelectorAll<HTMLElement>('[role="menuitem"]'));
		const currentIdx = menuItems.indexOf(document.activeElement as HTMLElement);

		switch (e.key) {
			case 'ArrowDown':
				e.preventDefault();
				menuItems[(currentIdx + 1) % menuItems.length]?.focus();
				break;
			case 'ArrowUp':
				e.preventDefault();
				menuItems[(currentIdx - 1 + menuItems.length) % menuItems.length]?.focus();
				break;
		}
	}

	function handleItemClick(action: () => void) {
		action();
		onClose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="context-backdrop" role="presentation" onclick={onClose}></div>
<!-- svelte-ignore a11y_interactive_supports_focus a11y_click_events_have_key_events -->
<div
	class="context-menu"
	role="menu"
	bind:this={menuEl}
	style="left: {x}px; top: {y}px"
	onclick={(e) => e.stopPropagation()}
>
	{#each items as item}
		{#if item.separator}
			<div class="separator" role="separator"></div>
		{:else}
			<button class="menu-item" role="menuitem" onclick={() => handleItemClick(item.action)}>
				{item.label}
			</button>
		{/if}
	{/each}
</div>

<style>
	.context-backdrop {
		position: fixed;
		inset: 0;
		z-index: var(--z-context);
	}

	@keyframes menu-in {
		from { opacity: 0; transform: scale(0.96); }
		to { opacity: 1; transform: scale(1); }
	}

	.context-menu {
		position: fixed;
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		box-shadow: var(--shadow-context);
		padding: 4px 0;
		min-width: 160px;
		z-index: calc(var(--z-context) + 1);
		animation: menu-in 100ms cubic-bezier(0.25, 1, 0.5, 1);
		will-change: opacity, transform;
	}

	.menu-item {
		display: block;
		width: 100%;
		padding: 5px 12px;
		background: none;
		border: none;
		color: var(--text-primary);
		font-size: 12px;
		font-family: var(--font-ui);
		text-align: left;
		cursor: pointer;
	}

	.menu-item:hover,
	.menu-item:focus-visible {
		background: var(--accent-subtle);
		color: var(--accent-hover);
		outline: none;
	}

	.separator {
		height: 1px;
		background: var(--border-subtle);
		margin: 4px 0;
	}
</style>
