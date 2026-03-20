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

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	function handleItemClick(action: () => void) {
		action();
		onClose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="context-backdrop" onclick={onClose}>
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div class="context-menu" style="left: {x}px; top: {y}px" onclick={(e) => e.stopPropagation()}>
		{#each items as item}
			{#if item.separator}
				<div class="separator"></div>
			{:else}
				<button class="menu-item" onclick={() => handleItemClick(item.action)}>
					{item.label}
				</button>
			{/if}
		{/each}
	</div>
</div>

<style>
	.context-backdrop {
		position: fixed;
		inset: 0;
		z-index: 1000;
	}

	.context-menu {
		position: fixed;
		background: var(--bg-elevated);
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
		padding: 4px 0;
		min-width: 160px;
		z-index: 1001;
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

	.menu-item:hover {
		background: var(--accent-subtle);
		color: var(--accent-hover);
	}

	.separator {
		height: 1px;
		background: var(--border-subtle);
		margin: 4px 0;
	}
</style>
